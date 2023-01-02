use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, VecDeque},
    time::Duration,
};

#[cfg(target_arch = "wasm32")]
use instant::Instant;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

// use time::OffsetDateTime;

use crate::{
    config::{self, Class, EquipmentPreset, Race, Stat},
    lingo::{self, act_name, definite, generate_name, indefinite},
    rand::{Rand, SliceExt},
};

pub const fn level_up_time(level: usize) -> Duration {
    Duration::from_secs((20 * level * 60) as _)
}

pub struct Simulation {
    pub player: Player,
    pub time_scale: f32,
    last: Instant,
}

impl Simulation {
    const FLAVOR_TASKS: &[(&'static str, Duration)] = &[
        (
            "Experiencing an enigmatic and foreboding night vision",
            Duration::from_millis(10000),
        ),
        (
            "Much is revealed about the wise old man you'd underestimated",
            Duration::from_millis(6000),
        ),
        (
            "A shocking series of events leaves you alone and bewildered, but resolute",
            Duration::from_millis(6000),
        ),
        (
            "Drawing upon an unrealized reserve of determination, you set out on a long and dangerous journey",
            Duration::from_millis(4000),
        ),
    ];

    pub fn new(player: Player) -> Self {
        Self {
            player,
            time_scale: 1.0,
            last: Instant::now(),
        }
    }

    pub fn tick(&mut self, rng: &Rand) {
        let dt = self.last.elapsed().as_secs_f32() * self.time_scale;
        self.last = Instant::now();

        self.player.elapsed += dt;

        if self.player.task.is_none() {
            self.player
                .set_task(Task::regular("Loading", Duration::from_millis(2000)));

            self.player.queue.extend(
                Self::FLAVOR_TASKS
                    .iter()
                    .map(|(title, duration)| Task::regular(*title, *duration)),
            );

            self.player.queue.push_back(Task::plot(
                format!("Loading {}", lingo::act_name(1)),
                Duration::from_millis(2000),
            ));
            self.player.quest_book.plot.reset(28.0);
            return;
        }

        if !self.player.task_bar.is_done() {
            self.player.task_bar.increment(dt);
            return;
        }

        let gain = matches!(
            self.player.task,
            Some(Task {
                kind: TaskKind::Kill { .. },
                ..
            })
        );

        if !gain {
            self.dequeue(rng);
            return;
        }

        if self.player.exp_bar.is_done() {
            self.player.level_up(rng)
        } else {
            self.player.exp_bar.increment(self.player.task_bar.max)
        }

        if self.player.quest_book.act() >= 1 {
            if self.player.quest_book.quest.is_done()
                || self.player.quest_book.current_quest().is_none()
            {
                self.complete_quest(rng);
            } else {
                self.player
                    .quest_book
                    .quest
                    .increment(self.player.task_bar.max)
            }
        }

        if self.player.quest_book.plot.is_done() {
            self.cinematic(rng);
        } else {
            self.player
                .quest_book
                .plot
                .increment(self.player.task_bar.max)
        }

        self.dequeue(rng);
    }

    pub fn dequeue(&mut self, rng: &Rand) {
        while self.player.task_bar.is_done() {
            let task = self
                .player
                .task
                .as_ref()
                .expect("a player should always be on a task");

            let old = task.clone();

            match &task.kind {
                // NPC
                TaskKind::Kill {
                    monster: Some(monster),
                } if monster.item.is_none() => {
                    self.player.choose_item(rng);
                }

                TaskKind::Kill {
                    monster:
                        Some(config::Monster {
                            name,
                            item: Some(item),
                            ..
                        }),
                } => {
                    let item = format!("{} {}", name, item).to_lowercase();
                    self.player.inventory.add_item(item, 1);
                }

                TaskKind::Buy => {
                    self.player
                        .inventory
                        .add_gold(-self.player.equipment_price());
                    self.player.choose_equipment(rng)
                }

                task @ TaskKind::HeadingToMarket | task @ TaskKind::Sell
                    if !self.player.inventory.is_empty() =>
                {
                    if matches!(task, TaskKind::Sell) {
                        let item = &self.player.inventory[0];
                        let mut amount = item.quantity * self.player.level;
                        if item.name.contains(" of ") {
                            amount *= 1 + rng.below_low(10) * (1 + rng.below_low(self.player.level))
                        }
                        self.player.inventory.pop();
                        self.player.inventory.add_gold(amount as _);
                    }

                    if !self.player.inventory.is_empty() {
                        let item = &self.player.inventory[self.player.inventory.len() - 1];
                        self.player.set_task(Task::sell(
                            format!("Selling {}", indefinite(&item.name, item.quantity)),
                            Duration::from_millis(1000),
                        ));
                        break;
                    }
                }

                TaskKind::Plot => self.complete_act(rng),

                _ => {}
            }

            if self.player.inventory.encumbrance.is_done() {
                self.player.set_task(Task::heading_to_market(
                    "Heading to market to sell loot",
                    Duration::from_millis(4000),
                ))
            } else if !self.player.queue.is_empty() {
                let task = self.player.queue.pop_back().unwrap();
                self.player.set_task(task);
            } else if !matches!(old.kind, TaskKind::Kill { .. } | TaskKind::HeadingOut) {
                if self.player.inventory.gold > self.player.equipment_price() {
                    self.player.set_task(Task::buy(
                        "Negotiating purchase of better equipment",
                        Duration::from_millis(5000),
                    ))
                } else {
                    self.player.set_task(Task::heading_out(
                        "Heading out into the world",
                        Duration::from_millis(4000),
                    ))
                }
            } else {
                self.player.set_task(Task::monster(
                    self.player.level as _,
                    self.player.quest_book.monster.clone(),
                    rng,
                ))
            }
        }
    }

    pub fn complete_act(&mut self, rng: &Rand) {
        self.player.quest_book.next_act();
        let max = (60 * 60 * (1 + 5 * self.player.quest_book.act)) as f32;

        self.player.quest_book.plot.reset(max);

        if self.player.quest_book.act() > 1 {
            self.player.choose_item(rng);
            self.player.choose_equipment(rng);
        }
    }

    pub fn complete_quest(&mut self, rng: &Rand) {
        self.player
            .quest_book
            .quest
            .reset((50 + rng.below_low(1000)) as f32);
        if let Some(quest) = self.player.quest_book.current_quest() {
            [
                Player::choose_item,
                Player::choose_spell,
                Player::choose_equipment,
                Player::choose_stat,
            ]
            .choice(rng)(&mut self.player, rng);
        }

        self.player.quest_book.monster.take();

        let caption = match rng.below(5) {
            0 => {
                let monster = unnamed_monster(self.player.level, 3, rng);
                let caption = format!("Exterminate {}", definite(&monster.name, 2));
                self.player.quest_book.monster.replace(monster);
                caption
            }
            1 => {
                format!("Seek {}", definite(&interesting_item(rng), 1))
            }
            2 => {
                format!("Deliver this {}", boring_item(rng))
            }
            3 => {
                format!("Fetch me {}", indefinite(boring_item(rng), 1))
            }
            4 => {
                let monster = unnamed_monster(self.player.level, 1, rng);
                format!("Placate {}", definite(&monster.name, 2))
            }
            _ => unreachable!(),
        };

        self.player.quest_book.add_quest(&caption);
    }

    pub fn cinematic(&mut self, rng: &Rand) {
        trait Queue {
            fn enqueue(&mut self, task: Task, rng: &Rand);
        }

        impl Queue for Simulation {
            fn enqueue(&mut self, task: Task, rng: &Rand) {
                self.player.queue.push_back(task);
                self.dequeue(rng);
            }
        }

        match rng.below(3) {
            0 => {
                for (description, duration) in [
                    (
                        "Exhausted, you arrive at a friendly oasis in a hostile land",
                        1000,
                    ),
                    ("You greet old friends and meet new allies", 2000),
                    ("You are privy to a council of powerful do-gooders", 2000),
                    ("There is much to be done, you are chosen!", 1000),
                ] {
                    self.enqueue(Task::regular(description, Duration::from_millis(1000)), rng)
                }
            }
            1 => {
                self.enqueue(
                    Task::regular(
                        "Your quarry is in sigh, but a mightly enemy bars your path!",
                        Duration::from_millis(1000),
                    ),
                    rng,
                );

                let nemesis = named_monster(self.player.level + 3, rng);
                self.enqueue(
                    Task::regular(
                        format!("A desperate struggle commences with {nemesis}"),
                        Duration::from_millis(4000),
                    ),
                    rng,
                );

                let mut s = rng.below(3);
                for i in 1.. {
                    if i > rng.below((1 + self.player.quest_book.act() + 1) as _) {
                        break;
                    }
                    s += 1 + rng.below(2);
                    match s % 3 {
                        0 => self.enqueue(
                            Task::regular(
                                format!("Locked in grim combat with {nemesis}"),
                                Duration::from_millis(2000),
                            ),
                            rng,
                        ),
                        1 => self.enqueue(
                            Task::regular(
                                format!("{nemesis} seems to have the upper hand"),
                                Duration::from_millis(1000),
                            ),
                            rng,
                        ),
                        2 => self.enqueue(
                            Task::regular(
                                format!("You seem to gain the advantage over {nemesis}"),
                                Duration::from_millis(2000),
                            ),
                            rng,
                        ),
                        _ => unreachable!(),
                    }
                }

                self.enqueue(
                    Task::regular(
                        format!("Victory! {nemesis} is slain! Exhauted, you lose consciousness"),
                        Duration::from_millis(3000),
                    ),
                    rng,
                );

                self.enqueue(
                    Task::regular(
                        "You awake in a friendly place, but the road awaits",
                        Duration::from_millis(2000),
                    ),
                    rng,
                );
            }
            2 => {
                let nemesis = impressive_npc(rng);
                for (description, duration) in [
                    (
                        format!(
                            "Oh sweet relief! You've reached the protection of the good {nemesis}"
                        ),
                        2000,
                    ),
                    (
                        format!(
                        "There is rejoicing, and an unnerving encounter with {nemesis} in private"
                    ),
                        3000,
                    ),
                    (
                        format!("You forgot your {} and go back to get it", boring_item(rng)),
                        2000,
                    ),
                    (
                        String::from("What's this!? Your overhead something shocking!"),
                        2000,
                    ),
                    (format!("Could {nemesis} be a dirty double-dealer?"), 2000),
                    (
                        String::from(
                            "Who can possibly be trusted with this new?! ... Oh yes, of course.",
                        ),
                        3000,
                    ),
                ] {
                    self.enqueue(
                        Task::regular(description, Duration::from_millis(duration)),
                        rng,
                    )
                }
            }
            _ => unreachable!(),
        };

        self.enqueue(
            Task::plot(
                format!("Loading {}", act_name(self.player.quest_book.act() + 1)),
                Duration::from_millis(1000),
            ),
            rng,
        )
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Task {
    pub description: Cow<'static, str>,
    pub duration: Duration,
    pub kind: TaskKind,
}

impl Task {
    pub fn regular(description: impl Into<Cow<'static, str>>, duration: Duration) -> Self {
        Self {
            description: description.into(),
            duration,
            kind: TaskKind::Regular,
        }
    }

    pub fn plot(description: impl Into<Cow<'static, str>>, duration: Duration) -> Self {
        Self {
            description: description.into(),
            duration,
            kind: TaskKind::Plot,
        }
    }

    pub fn sell(description: impl Into<Cow<'static, str>>, duration: Duration) -> Self {
        Self {
            description: description.into(),
            duration,
            kind: TaskKind::Sell,
        }
    }

    pub fn heading_to_market(
        description: impl Into<Cow<'static, str>>,
        duration: Duration,
    ) -> Self {
        Self {
            description: description.into(),
            duration,
            kind: TaskKind::HeadingToMarket,
        }
    }

    pub fn heading_out(description: impl Into<Cow<'static, str>>, duration: Duration) -> Self {
        Self {
            description: description.into(),
            duration,
            kind: TaskKind::HeadingOut,
        }
    }

    pub fn buy(description: impl Into<Cow<'static, str>>, duration: Duration) -> Self {
        Self {
            description: description.into(),
            duration,
            kind: TaskKind::Buy,
        }
    }

    pub fn monster(
        player_level: isize,
        quest_monster: Option<config::Monster>,
        rng: &Rand,
    ) -> Self {
        let mut level = player_level;
        for _ in 0..player_level {
            if rng.odds(2, 5) {
                level += rng.below(2) as isize * 2 - 1
            }
        }

        let mut level = level.max(1);

        let mut is_definite = false;
        let mut monster = Option::<config::Monster>::None;

        let task_level: isize;
        let result;

        if rng.odds(1, 25) {
            let race = config::RACES.choice(rng);
            if rng.odds(1, 2) {
                result = format!("passing {} {}", race.name, config::CLASSES.choice(rng).name);
            } else {
                result = format!(
                    "{} {} the {}",
                    config::TITLES.choice_low(rng),
                    generate_name(None, rng),
                    race.name
                );
                is_definite = true;
            }
            task_level = level;
        } else if quest_monster.is_some() && rng.odds(1, 4) {
            let quest_monster = quest_monster.unwrap();
            result = quest_monster.name.to_string();
            task_level = quest_monster.level as isize;
            monster.replace(quest_monster);
        } else {
            monster.replace(unnamed_monster(level as _, 5, rng));
            let monster = monster.as_ref().unwrap();
            result = monster.name.to_string();
            task_level = monster.level as isize
        }

        let mut qty = 1;
        if level - task_level > 10 {
            qty = (level + rng.below(task_level.max(1) as usize) as isize) / (task_level).max(1);
            qty = qty.max(1);
            level /= qty
        }

        use crate::lingo::*;

        let mut result = match () {
            _ if level - task_level <= -10 => format!("imaginary {result}"),
            _ if level - task_level < -5 => {
                let i = 10 + level - task_level;
                let i = 5 - rng.below((i + 1) as _);
                sick(i, &young((task_level - level - (i as isize)) as _, &result)).to_string()
            }
            _ if level - task_level < 0 && rng.odds(1, 2) => {
                sick((level - task_level) as _, &result).to_string()
            }
            _ if level - task_level < 0 => young((level - task_level) as _, &result).to_string(),
            _ if level - task_level >= -10 => {
                format!("unreal {result}")
            }
            _ if level - task_level > 5 => {
                let i = 10 - (level - task_level);
                let i = 5 - rng.below((i + 1) as _);
                big(
                    i,
                    &special((task_level - level - (i as isize)) as _, &result),
                )
                .to_string()
            }
            _ if level - task_level > 0 && rng.odds(1, 2) => {
                big((level - task_level) as _, &result).to_string()
            }
            _ if level - task_level > 0 => special((level - task_level) as _, &result).to_string(),

            _ => unreachable!(),
        };

        let task_level = level;
        let level = task_level * qty;

        if !is_definite {
            result = indefinite(&result, qty as _)
        }

        Self {
            description: format!("Attacking {result}").into(),
            duration: Duration::from_millis(((2 * 3 * level * 1000) / player_level) as _),
            kind: TaskKind::Kill { monster },
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum TaskKind {
    Kill { monster: Option<config::Monster> },
    Buy,
    HeadingOut,
    HeadingToMarket,
    Sell,
    Regular,
    Plot,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Stats {
    pub(crate) values: Vec<(Stat, usize)>,
}

impl Stats {
    pub fn new(iter: impl IntoIterator<Item = (Stat, usize)>) -> Self {
        let mut map = BTreeMap::new();
        for (k, v) in iter.into_iter().chain(
            config::ALL_STATS
                .into_iter()
                .zip(std::iter::repeat(0_usize)),
        ) {
            map.entry(k).or_insert(v);
        }

        Self {
            values: map.into_iter().collect(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &(Stat, usize)> + ExactSizeIterator + '_ {
        self.values.iter()
    }

    pub fn best(&self) -> Stat {
        debug_assert!(!self.values.is_empty(), "atleast a single stat must exist");
        self.iter()
            .max_by_key(|(_, v)| *v)
            .map(|(k, _)| *k)
            .unwrap()
    }

    pub fn best_prime(&self) -> Stat {
        debug_assert!(!self.values.is_empty(), "atleast a single stat must exist");
        self.iter()
            .filter(|(k, _)| config::PRIME_STATS.contains(k))
            .max_by_key(|(_, v)| *v)
            .map(|(k, _)| *k)
            .unwrap()
    }

    pub fn increment(&mut self, stat: Stat, quantity: usize) {
        *self
            .values
            .iter_mut()
            .find_map(|(s, q)| (*s == stat).then_some(q))
            .unwrap_or_else(|| panic!("stat does not exist: {stat:?}")) += quantity;
    }
}

impl std::ops::Index<Stat> for Stats {
    type Output = usize;
    fn index(&self, index: Stat) -> &Self::Output {
        self.values
            .iter()
            .find_map(|(s, q)| (*s == index).then_some(q))
            .unwrap_or_else(|| panic!("stat does not exist: {index:?}"))
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct QuestBook {
    quests: VecDeque<String>,
    act: i32,
    monster: Option<config::Monster>,
    pub plot: Bar,
    pub quest: Bar,
}

impl QuestBook {
    const MAX_QUESTS: usize = 100;

    pub fn new() -> Self {
        Self {
            quests: VecDeque::new(),
            act: 0,
            monster: None,
            plot: Bar::with_max(1.0),
            quest: Bar::with_max(1.0),
        }
    }

    pub fn next_act(&mut self) {
        self.act += 1;
    }

    pub fn add_quest(&mut self, quest: &str) {
        while self.quests.len() >= Self::MAX_QUESTS {
            self.quests.pop_front();
        }
        self.quests.push_back(quest.to_string());
    }

    pub fn current_quest(&self) -> Option<&str> {
        self.quests.back().map(|s| &**s)
    }

    pub const fn act(&self) -> i32 {
        self.act
    }

    pub fn quests(&self) -> impl Iterator<Item = &str> + ExactSizeIterator {
        self.quests.iter().map(|s| &**s)
    }

    pub fn completed_quests(&self) -> impl Iterator<Item = &str> + ExactSizeIterator {
        let n = self.quests.len().saturating_sub(1);
        self.quests().take(n)
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Spell {
    name: String,
    level: i32,
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct SpellBook {
    spells: Vec<Spell>,
}

impl SpellBook {
    pub fn add(&mut self, name: &str, level: i32) {
        for spell in &mut self.spells {
            if spell.name == name {
                spell.level += level;
                return;
            }
        }

        self.spells.push(Spell {
            name: String::from(name),
            level,
        });
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, i32)> + ExactSizeIterator {
        self.spells
            .iter()
            .map(|Spell { name, level }| (&**name, *level))
    }

    pub fn best(&self) -> Option<&Spell> {
        self.spells.iter().max_by_key(|Spell { level, .. }| level)
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InventoryItem {
    name: String,
    quantity: usize,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Inventory {
    capacity: usize,
    gold: isize,
    items: Vec<InventoryItem>,
    pub encumbrance: Bar,
}

impl Inventory {
    pub const fn new(capacity: usize) -> Self {
        Self {
            capacity,
            encumbrance: Bar::with_max(capacity as _),
            gold: 0,
            items: Vec::new(),
        }
    }

    pub fn items(&self) -> impl Iterator<Item = (&String, &usize)> + ExactSizeIterator {
        self.items
            .iter()
            .map(|InventoryItem { name, quantity }| (name, quantity))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn set_capacity(&mut self, cap: usize) {
        self.capacity = cap;
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn gold(&self) -> isize {
        self.gold
    }

    pub fn add_gold(&mut self, quantity: isize) {
        self.gold += quantity;
    }

    pub fn add_item(&mut self, item: impl ToString + AsRef<str>, quantity: usize) {
        if let Some(qty) = self
            .items
            .iter_mut()
            .find_map(|InventoryItem { name, quantity }| {
                (&**name == item.as_ref()).then_some(quantity)
            })
        {
            *qty += quantity;
            return;
        }

        self.items.push(InventoryItem {
            name: item.to_string(),
            quantity,
        });

        self.update_bar();
    }

    pub fn pop(&mut self) {
        let item = self.items.pop().expect("inventory not empty");
        self.update_bar();
    }

    fn update_bar(&mut self) {
        self.encumbrance.pos = self
            .items
            .iter()
            .map(|InventoryItem { quantity, .. }| quantity)
            .sum::<usize>() as f32;
    }
}

impl std::ops::Index<usize> for Inventory {
    type Output = InventoryItem;

    fn index(&self, index: usize) -> &Self::Output {
        &self.items[index]
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Equipment {
    items: BTreeMap<config::Equipment, String>,
    best: String,
}

impl Default for Equipment {
    fn default() -> Self {
        Self {
            items: [
                (config::Equipment::Weapon, "Sharp Rock".into()),
                (config::Equipment::Hauberk, "-3 Burlap".into()),
            ]
            .into_iter()
            .collect(),
            best: "Sharp Rock".into(),
        }
    }
}

impl Equipment {
    pub fn add(&mut self, ty: config::Equipment, name: impl ToString) {
        *self.items.entry(ty).or_default() = name.to_string();

        self.best = format!(
            "{name} {item}",
            name = name.to_string(),
            item = if matches!(ty, config::Equipment::Weapon | config::Equipment::Shield) {
                ""
            } else {
                ty.as_str()
            }
        )
    }

    pub fn iter(&self) -> impl Iterator<Item = (config::Equipment, &str)> + ExactSizeIterator {
        self.items.iter().map(|(eq, name)| (*eq, &**name))
    }
}

#[derive(Copy, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Bar {
    pub pos: f32,
    pub max: f32,
}

impl Bar {
    pub const fn with_max(max: f32) -> Self {
        Self { pos: 0.0, max }
    }

    pub fn remaining(&self) -> f32 {
        self.max - self.pos
    }

    pub fn increment(&mut self, pos: f32) {
        self.pos = f32::min(self.pos + pos, self.max);
    }

    pub fn is_done(&self) -> bool {
        self.pos >= self.max
    }

    pub fn reset(&mut self, max: f32) {
        self.max = max;
        self.pos = 0.0;
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Player {
    pub name: String,

    // #[serde(with = "time::serde::iso8601")]
    // birthday: OffsetDateTime,
    pub race: Race,
    pub class: Class,
    pub level: usize,

    pub stats: Stats,
    pub elapsed: f32,

    pub quest_book: QuestBook,
    pub spell_book: SpellBook,
    pub inventory: Inventory,
    pub equipment: Equipment,

    pub task: Option<Task>,
    pub queue: VecDeque<Task>,

    pub task_bar: Bar,
    pub exp_bar: Bar,
}

impl Player {
    pub fn new(name: impl Into<String>, race: Race, class: Class, stats: Stats) -> Self {
        let (spell_book, equipment, task, queue) = <_>::default();

        Self {
            inventory: Inventory::new(10 + stats[Stat::Strength]),
            name: name.into(),
            // birthday: OffsetDateTime::now_utc(),
            elapsed: 0.0,
            level: 1,

            race,
            class,
            stats,

            quest_book: QuestBook::new(),
            spell_book,
            equipment,
            task,
            queue,

            task_bar: Bar::with_max(1.0),
            exp_bar: Bar::with_max(level_up_time(1).as_secs() as f32),
        }
    }

    pub fn total_queue_time(&self) -> f32 {
        self.queue
            .iter()
            .map(|task| task.duration.as_secs_f32())
            .sum()
    }

    pub fn set_task(&mut self, task: Task) {
        self.task_bar.reset(task.duration.as_secs_f32());
        self.task.replace(task);
    }

    pub const fn equipment_price(&self) -> isize {
        // the algorithm
        (5 * self.level.pow(2) + 10 * self.level + 20) as _
    }

    pub fn level_up(&mut self, rng: &Rand) {
        self.level += 1;

        let adjust = |n| n / 3 + 1 + rng.below(4);
        for (amount, stat) in [
            (self.stats[Stat::Condition], Stat::HpMax),
            (self.stats[Stat::Intelligence], Stat::MpMax),
        ] {
            self.stats.increment(stat, adjust(amount));
        }

        self.choose_stat(rng);
        self.choose_stat(rng);
        self.choose_spell(rng);

        self.exp_bar
            .reset(level_up_time(self.level).as_secs() as f32)
    }

    fn choose_stat(&mut self, rng: &Rand) {
        let stat = if rng.odds(1, 2) {
            *config::ALL_STATS.choice(rng)
        } else {
            let mut t = rng.below(self.stats.iter().map(|(_, s)| s.pow(2)).sum());
            self.stats
                .iter()
                .find_map(|(stat, value)| match t.checked_sub(value.pow(2)) {
                    Some(val) => {
                        t = val;
                        None
                    }
                    None => Some(stat),
                })
                .copied()
                .expect("chose a stat")
        };

        self.stats.increment(stat, 1);
        if stat == Stat::Strength {
            self.inventory.set_capacity(10 + self.stats[Stat::Strength])
        }
    }

    fn choose_spell(&mut self, rng: &Rand) {
        let choice = self.stats[Stat::Wisdom] + self.level;
        let index = rng.below_low(choice).min(config::SPELLS.len() - 1);
        self.spell_book.add(config::SPELLS[index], 1)
    }

    fn choose_equipment(&mut self, rng: &Rand) {
        use config::Equipment::*;
        let (stuff, better, worse) = match [
            Weapon, Shield, Helm, Hauberk, Brassairts, //
            Vambraces, Gauntlets, Guisses, Greaves, Sollerets,
        ]
        .choice(rng)
        {
            Weapon => (
                config::WEAPONS,
                config::OFFENSE_ATTRIBUTE,
                config::OFFENSE_QUIRK,
            ),
            Shield => (
                config::SHIELDS,
                config::DEFENSE_ATTRIBUTE,
                config::DEFENSE_QUIRK,
            ),
            _ => (
                config::ARMORS,
                config::DEFENSE_ATTRIBUTE,
                config::DEFENSE_QUIRK,
            ),
        };

        let equipment = pick_equipment(stuff, self.level as _, rng);
        let mut name = equipment.name.to_string();

        let mut positive = self.level as i32 - equipment.quality;
        let pool = if positive < 0 { worse } else { better };

        let mut count = 0;
        let mut modifier;
        while count < 2 && positive > 0 {
            modifier = rng.choice(pool);
            if modifier.name == name {
                break;
            }

            if positive.abs() < modifier.quality.abs() {
                break;
            }

            name = format!("{} {name}", modifier.name);
            positive -= modifier.quality;
            count += 1
        }

        name = match positive {
            0 => name,
            _ => format!(
                "{delta}{positive} {name}",
                delta = if positive > 0 { "+" } else { "" }
            ),
        };

        self.equipment.add(
            *[
                Weapon, Shield, Helm, Hauberk, Brassairts, Vambraces, Gauntlets, Guisses, Greaves,
                Sollerets,
            ]
            .choice(rng),
            name,
        );
    }

    fn choose_item(&mut self, rng: &Rand) {
        self.inventory.add_item(special_item(rng), 1);
    }
}

fn special_item(rng: &Rand) -> String {
    format!(
        "{} of {}",
        interesting_item(rng),
        config::ITEM_PREPOSITION.choice(rng)
    )
}

fn interesting_item(rng: &Rand) -> String {
    format!(
        "{} {}",
        config::ITEM_ATTRIBUTES.choice(rng),
        config::SPECIALS.choice(rng)
    )
}

fn boring_item(rng: &Rand) -> &'static str {
    config::BORING_ITEMS.choice(rng)
}

fn impressive_npc(rng: &Rand) -> String {
    let title = config::IMPRESSIVE_TITLES.choice(rng);
    let (suffix, name) = if rng.odds(1, 3) {
        ("of the ", Cow::from(&*config::RACES.choice(rng).name))
    } else {
        ("of ", Cow::from(generate_name(None, rng)))
    };

    format!("{title} {suffix} {name}")
}

fn unnamed_monster(level: usize, attempts: usize, rng: &Rand) -> config::Monster {
    let mut monster = config::MONSTERS.choice(rng);

    for _ in 0..attempts {
        let alt = config::MONSTERS.choice(rng);
        if level.saturating_sub(alt.level) < level.saturating_sub(monster.level) {
            monster = alt;
        }
    }

    monster.clone()
}

fn named_monster(level: usize, rng: &Rand) -> String {
    let monster = unnamed_monster(level, 4, rng);
    format!("{} the {}", generate_name(None, rng), monster.name)
}

fn pick_equipment(source: &[config::EquipmentPreset], goal: i32, rng: &Rand) -> EquipmentPreset {
    let mut out = rng.choice(source);
    for _ in 0..5 {
        let alt = rng.choice(source);
        if (goal - alt.quality).abs() < (goal - out.quality).abs() {
            out = alt;
        }
    }
    out.clone()
}

#[derive(Default)]
pub struct StatsBuilder {
    history: VecDeque<Stats>,
}

impl StatsBuilder {
    const MAX_HISTORY: usize = 10;

    pub fn roll(&mut self, rng: &Rand) -> Stats {
        const MAX: usize = config::PRIME_STATS.len();

        let mut values: HashMap<Stat, usize> = config::PRIME_STATS
            .into_iter()
            .map(|stat| (stat, 3 + (0..3).map(|_| rng.below(MAX)).sum::<usize>()))
            .collect();

        for (stat, base) in [
            (Stat::HpMax, Stat::Condition),
            (Stat::MpMax, Stat::Intelligence),
        ] {
            values.insert(stat, rng.below(config::ALL_STATS.len()) + values[&base]);
        }

        let stats = Stats::new(values.into_iter());
        while self.history.len() >= Self::MAX_HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(stats.clone());
        stats
    }

    pub fn has_history(&self) -> bool {
        self.history.len() > 1
    }

    pub fn unroll(&mut self) -> Stats {
        if self.history.len() > 1 {
            self.history.pop_back();
        }
        self.history.back().cloned().unwrap()
    }
}
