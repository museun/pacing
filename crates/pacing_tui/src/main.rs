use std::sync::{Arc, Mutex, MutexGuard};

use cursive::{
    align::HAlign,
    event::Event,
    theme::{Color, Palette, PaletteColor, Theme},
    view::Nameable,
    views::{DummyView, LinearLayout, ListView, OnEventView, Panel, ProgressBar, TextView},
    Cursive, View,
};

use log::RecordBuilder;
use pacing_core::{
    config::{CLASSES, RACES},
    format::Roman,
    lingo::generate_name,
    mechanics::{Bar, Player, Simulation, StatsBuilder},
    Rand, SliceExt,
};

fn default_palette() -> Palette {
    use PaletteColor::*;
    [
        Background,
        Shadow,
        View,
        Primary,
        Secondary,
        Tertiary,
        TitlePrimary,
        TitleSecondary,
        Highlight,
        HighlightInactive,
        HighlightText,
    ]
    .into_iter()
    .zip(std::iter::repeat(Color::TerminalDefault))
    .fold(Palette::default(), |mut p, (k, v)| {
        p[k] = v;
        p
    })
}

#[derive(Clone)]
struct App {
    simulation: Arc<Mutex<Simulation>>,
}

impl App {
    fn get(&self) -> AppRef<'_> {
        AppRef {
            simulation: self.simulation.lock().unwrap(),
        }
    }
}

struct AppRef<'a> {
    simulation: MutexGuard<'a, Simulation>,
}

impl AppRef<'_> {
    fn make_progress_bar(bar: &Bar) -> ProgressBar {
        let mut pb = ProgressBar::new()
            .min(0 as usize)
            .with_label(|_, _| String::new())
            .with_color(Color::Dark(cursive::theme::BaseColor::Red))
            .max(bar.max as _);
        pb.set_value(bar.pos as _);
        pb
    }
}

impl AppRef<'_> {
    fn display(&mut self) -> impl View {
        LinearLayout::vertical()
            .child(
                LinearLayout::horizontal()
                    .child(self.left_panel())
                    .child(self.middle_panel())
                    .child(self.right_view()),
            )
            .child(self.bottom_view())
    }

    fn left_panel(&self) -> impl View {
        LinearLayout::vertical()
            .child(self.character_sheet())
            .child(self.spell_book())
    }

    fn middle_panel(&self) -> impl View {
        LinearLayout::vertical()
            .child(self.equipment_list())
            .child(self.inventory_list())
    }

    fn right_view(&self) -> impl View {
        LinearLayout::vertical()
            .child(self.plot_development())
            .child(DummyView)
            .child(self.quest_list())
    }

    fn bottom_view(&self) -> impl View {
        let mut ll = LinearLayout::vertical();
        if let Some(task) = &self.simulation.player.task {
            ll.add_child(TextView::new(&*task.description))
        }
        ll.child(self.progress_bar())
    }

    fn equipment_list(&self) -> impl View {
        let mut lv = ListView::new();

        for (item, stat) in self.simulation.player.equipment.iter() {
            lv.add_child(item.as_str(), TextView::new(stat).h_align(HAlign::Right))
        }

        Panel::new(lv).title("Equipment")
    }

    fn inventory_list(&self) -> impl View {
        let mut lv = ListView::new().child("Item", TextView::new("Qty")).child(
            "Gold",
            TextView::new(self.simulation.player.inventory.gold().to_string())
                .h_align(HAlign::Right),
        );

        for (item, qty) in self.simulation.player.inventory.items() {
            lv.add_child(item, TextView::new(qty.to_string()).h_align(HAlign::Right))
        }

        Panel::new(
            LinearLayout::vertical().child(lv).child(DummyView).child(
                LinearLayout::vertical()
                    .child(TextView::new("Encumbrance"))
                    .child(self.encumbrance_bar()),
            ),
        )
        .title("Inventory")
    }

    fn plot_development(&self) -> impl View {
        fn format_act(act: i32) -> String {
            (act == 0)
                .then(|| "Prologue".to_string())
                .unwrap_or_else(|| format!("Act {}", Roman::from_i32(act)))
        }

        Panel::new({
            LinearLayout::vertical()
                .child(
                    (0..self.simulation.player.quest_book.act())
                        .map(format_act)
                        .fold(ListView::new(), |lv, act| {
                            lv.child(&format!("[x] {act}"), DummyView)
                        })
                        .child(
                            &format!(
                                "[ ] {current}",
                                current = format_act(self.simulation.player.quest_book.act())
                            ),
                            DummyView,
                        ),
                )
                .child(DummyView)
                .child(self.plot_bar())
        })
        .title("Plot development")
    }

    fn quest_list(&self) -> impl View {
        Panel::new({
            let mut lv = self
                .simulation
                .player
                .quest_book
                .completed_quests()
                .fold(ListView::new(), |lv, q| {
                    lv.child(&format!("[x] {q}"), DummyView)
                });
            if let Some(current) = self.simulation.player.quest_book.current_quest() {
                lv.add_child(&format!("[ ] {current}"), DummyView)
            }

            LinearLayout::vertical()
                .child(lv)
                .child(DummyView)
                .child(self.quest_bar())
        })
        .title("Quests")
    }

    fn character_sheet(&self) -> impl View {
        Panel::new(
            LinearLayout::vertical()
                .child(self.trait_sheet())
                .child(DummyView)
                .child(self.stat_sheet())
                .child(DummyView)
                .child(self.experience_bar()),
        )
        .title("Character sheet")
    }

    fn spell_book(&self) -> impl View {
        Panel::new({
            let mut lv =
                ListView::new().child("Spell", TextView::new("Level").h_align(HAlign::Right));
            for (spell, level) in self.simulation.player.spell_book.iter() {
                lv.add_child(
                    spell,
                    TextView::new(Roman::from_i32(level)).h_align(HAlign::Right),
                );
            }
            lv
        })
        .title("Spell book")
    }

    fn progress_bar(&self) -> impl View {
        Self::make_progress_bar(&self.simulation.player.task_bar)
    }

    fn experience_bar(&self) -> impl View {
        Self::make_progress_bar(&self.simulation.player.exp_bar)
    }

    fn encumbrance_bar(&self) -> impl View {
        Self::make_progress_bar(&self.simulation.player.inventory.encumbrance)
    }

    fn quest_bar(&self) -> impl View {
        Self::make_progress_bar(&self.simulation.player.quest_book.quest)
    }

    fn plot_bar(&self) -> impl View {
        Self::make_progress_bar(&self.simulation.player.quest_book.plot)
    }

    fn trait_sheet(&self) -> impl View {
        let mut ch = ListView::new().child("Trait", TextView::new("Value").h_align(HAlign::Right));

        for (trait_, value) in [
            ("Name", &*self.simulation.player.name),
            ("Level", &*self.simulation.player.level.to_string()),
            ("Class", &*self.simulation.player.class.name),
            ("Race", &*self.simulation.player.race.name),
        ] {
            ch.add_child(trait_, TextView::new(value).h_align(HAlign::Right))
        }
        ch
    }

    fn stat_sheet(&self) -> impl View {
        let mut stats =
            ListView::new().child("Stat", TextView::new("Value").h_align(HAlign::Right));
        for (k, v) in self.simulation.player.stats.iter() {
            stats.add_child(
                k.as_str(),
                TextView::new(v.to_string()).h_align(HAlign::Right),
            )
        }
        stats
    }
}

fn main() {
    let rng = Rand::new();

    let player = Player::new(
        generate_name(None, &rng),
        RACES.choice(&rng).clone(),
        CLASSES.choice(&rng).clone(),
        StatsBuilder::default().roll(&rng),
    );
    let mut app = App {
        simulation: Arc::new(Mutex::new(Simulation::new(player))),
    };

    app.get().simulation.time_scale = 10.0;

    let mut cursive = cursive::default();

    cursive.set_theme(Theme {
        shadow: false,
        borders: cursive::theme::BorderStyle::Simple,
        palette: default_palette(),
    });

    cursive.add_fullscreen_layer(
        OnEventView::new(app.get().display().with_name("main_view")).on_event(Event::Refresh, {
            let app = app.clone();
            move |cursive| {
                cursive.call_on_name("main_view", |v| *v = app.get().display());
            }
        }),
    );

    cursive.add_global_callback('1', Cursive::toggle_debug_console);
    cursive.add_global_callback('q', |s| s.quit());
    cursive.set_autorefresh(true);

    let mut cursive = cursive.into_runner();
    cursive.refresh();

    while cursive.is_running() {
        app.get().simulation.tick(&rng);

        cursive.step();
    }
}
