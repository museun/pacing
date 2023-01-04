use std::time::Duration;

use egui::{
    style::Margin, Align, Button, CentralPanel, Color32, Frame, Label, Layout, RichText, Rounding,
    ScrollArea, Sense, SidePanel, Stroke, TextEdit, TopBottomPanel,
};
use pacing_core::{Rand, SliceExt};
use tray_icon::TrayEvent;

use crate::{
    config,
    format::Roman,
    lingo::{act_name, generate_name},
    mechanics::{Player, Simulation, StatsBuilder},
    progress::Progress,
    view::View,
};

#[derive(Default)]
enum DetailsResult {
    Play,
    Close,
    #[default]
    Nothing,
}

#[derive(Default)]
enum CreationResult {
    Created,
    Cancel,
    #[default]
    Nothing,
}

#[derive(Default)]
enum SelectionResult {
    Selected(usize),
    Details(usize),
    Create,
    #[default]
    Nothing,
}

pub struct MainWindow {
    rng: Rand,
    view: View,
    is_visible: bool,
}

impl MainWindow {
    const SETTINGS_KEY: &'static str = concat!(env!("CARGO_PKG_NAME"), "_settings");
    const FRAME_RATE: Duration = Duration::from_millis(16);

    pub fn new(cc: &eframe::CreationContext) -> Self {
        // TODO seed this
        let rng = Rand::new();

        if let Some(storage) = cc.storage {
            if let Some(players) = eframe::get_value(storage, Self::SETTINGS_KEY) {
                return Self {
                    rng,
                    view: View::CharacterSelect { players },
                    is_visible: true,
                };
            }
        }

        let (player, stats_builder) = Self::make_new_character(&rng);
        Self {
            rng,
            view: View::CharacterCreation {
                player,
                stats_builder,
                players: vec![],
            },
            is_visible: true,
        }
    }

    fn success_button(text: impl Into<String>) -> Button {
        const SUCCESS_FILL: Color32 = Color32::from_rgb(0x21, 0x36, 0x54);
        const SUCCESS_TEXT: Color32 = Color32::from_rgb(0x8d, 0xb6, 0xf2);

        Button::new(RichText::new(text).color(SUCCESS_TEXT)).fill(SUCCESS_FILL)
    }

    fn caution_button(text: impl Into<String>) -> Button {
        const CAUTION_FILL: Color32 = Color32::from_rgb(0x57, 0x26, 0x22);
        const CAUTION_TEXT: Color32 = Color32::from_rgb(0xf2, 0x94, 0x94);

        Button::new(RichText::new(text).color(CAUTION_TEXT)).fill(CAUTION_FILL)
    }

    fn make_new_character(rng: &Rand) -> (Player, StatsBuilder) {
        let mut stats_builder = StatsBuilder::default();
        let player = Player::new(
            generate_name(None, rng),
            config::RACES.choice(rng).clone(),
            config::CLASSES.choice(rng).clone(),
            stats_builder.roll(rng),
        );

        (player, stats_builder)
    }

    const fn summary_stat_color(total: usize) -> Color32 {
        match total {
            total if total > 63 + 18 => Color32::RED,
            total if total > 4 * 18 => Color32::YELLOW,
            total if total <= 63 - 18 => Color32::LIGHT_GRAY,
            total if total <= 3 * 18 => Color32::GRAY,
            _ => Color32::WHITE,
        }
    }

    fn display_character_detail(player: &Player, ui: &mut egui::Ui) -> DetailsResult {
        let mut out = DetailsResult::default();
        ui.horizontal(|ui| {
            ui.heading(&player.name);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(Self::success_button("Play")).clicked() {
                    out = DetailsResult::Play;
                }
                if ui.add(Self::caution_button("Close")).clicked() {
                    out = DetailsResult::Close;
                }
            });
        });
        ui.separator();

        ScrollArea::vertical()
            .id_source("detail_list")
            .show(ui, |ui| {
                ui.heading("Details");
                ui.horizontal(|ui| {
                    ui.monospace("Level");
                    ui.label(player.level.to_string());
                });

                ui.horizontal(|ui| {
                    ui.monospace("Class");
                    ui.label(&*player.class.name);
                });

                ui.horizontal(|ui| {
                    ui.monospace("Race");
                    ui.label(&*player.race.name);
                });
            });

        ui.separator();
        ui.heading("Stats");

        for (k, v) in player.stats.iter() {
            if let config::Stat::HpMax = k {
                ui.separator();
            }

            ui.horizontal(|ui| {
                ui.monospace(k.as_str());
                ui.monospace(v.to_string());
            });
        }

        out
    }

    fn display_character_select(players: &mut Vec<Player>, ui: &mut egui::Ui) -> SelectionResult {
        let mut selection = SelectionResult::default();
        let mut remove = Option::<usize>::None;

        ScrollArea::vertical().show(ui, |ui| {
            for (i, player) in players.iter().enumerate() {
                let resp = Frame::none()
                    .inner_margin(Margin::same(6.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.heading(&player.name);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.add(Self::success_button("Play")).clicked() {
                                    selection = SelectionResult::Selected(i);
                                }

                                if ui.add(Self::caution_button("Delete")).clicked() {
                                    remove.replace(i);
                                }
                            });
                        });
                    })
                    .response
                    .interact(Sense::hover().union(Sense::click()));

                // TODO ignore mouse over buttons
                let resp = resp.on_hover_text_at_pointer("Click for details");

                if resp.hovered() {
                    ui.painter_at(resp.rect).rect_stroke(
                        resp.rect,
                        Rounding::none(),
                        ui.visuals().selection.stroke,
                    )
                }
                if resp.clicked() {
                    selection = SelectionResult::Details(i)
                }
            }
        });

        if let Some(index) = remove.take() {
            players.remove(index);
        }

        if ui.button("Create new character").clicked() {
            selection = SelectionResult::Create
        }

        selection
    }

    fn display_character_creation(
        player: &mut Player,
        stats_builder: &mut StatsBuilder,
        rng: &Rand,
        ui: &mut egui::Ui,
    ) -> CreationResult {
        fn make_frame(
            ui: &mut egui::Ui,
            label: &'static str,
            mut add_contents: impl FnMut(&mut egui::Ui),
        ) {
            Frame::none()
                .stroke(Stroke::new(1.0, ui.visuals().code_bg_color))
                .inner_margin(Margin::same(4.0))
                .show(ui, |ui| {
                    ScrollArea::vertical().id_source(label).show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(label);
                        });
                        ui.separator();
                        add_contents(ui)
                    });
                });
        }

        let mut created = CreationResult::default();
        TopBottomPanel::top("selection_panel")
            .show_separator_line(false)
            .resizable(false)
            .frame(Frame::none().outer_margin(Margin {
                left: 0.0,
                right: 0.0,
                top: 0.0,
                bottom: 4.0,
            }))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(TextEdit::singleline(&mut player.name).desired_width(100.0));

                    if ui.small_button("🎲").clicked() {
                        player.name = generate_name(None, rng);
                    }

                    ui.separator();

                    if ui.small_button("Roll").clicked() {
                        player.stats = stats_builder.roll(rng);
                    }

                    ui.add_enabled_ui(stats_builder.has_history(), |ui| {
                        if ui.small_button("Unroll").clicked() {
                            player.stats = stats_builder.unroll();
                        }
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(Self::success_button("Sold!")).clicked() {
                            created = CreationResult::Created
                        }
                        if ui.add(Self::caution_button("Cancel")).clicked() {
                            created = CreationResult::Cancel
                        }
                    });
                });
            });

        ui.columns(3, |ui| {
            make_frame(&mut ui[0], "Race", |ui| {
                for race in config::RACES {
                    if ui
                        .radio(player.race.name == race.name, &*race.name)
                        .clicked()
                    {
                        player.race = race.clone();
                    }
                }
            });

            make_frame(&mut ui[1], "Class", |ui| {
                for class in config::CLASSES {
                    if ui
                        .radio(player.class.name == class.name, &*class.name)
                        .clicked()
                    {
                        player.class = class.clone();
                    }
                }
            });

            let mut total = 0;

            make_frame(&mut ui[2], "Stats", |ui| {
                for (stat, qty) in player.stats.iter() {
                    if let config::Stat::HpMax = stat {
                        ui.separator();
                    }
                    ui.horizontal(|ui| {
                        ui.monospace(stat.as_str());
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            ui.monospace(qty.to_string());
                        });
                    });
                    if let config::Stat::HpMax | config::Stat::MpMax = stat {
                        continue;
                    }
                    total += qty;
                }

                ui.separator();
                ui.horizontal(|ui| {
                    ui.monospace("Total");
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.add(Label::new(
                            RichText::new(total.to_string())
                                .monospace()
                                .color(Self::summary_stat_color(total)),
                        ));
                    });
                });
            });
        });

        created
    }

    fn display_game(simulation: &mut Simulation, rng: &Rand, ctx: &egui::Context) {
        fn stroke(ui: &mut egui::Ui) -> Stroke {
            Stroke::new(
                ui.visuals().selection.stroke.width,
                ui.visuals().code_bg_color,
            )
        }

        fn make_frame(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
            Frame::none()
                .stroke(stroke(ui))
                .inner_margin(Margin::same(4.0))
                .show(ui, add_contents);
        }

        fn make_label(s: &str) -> Label {
            Label::new(RichText::new(s).monospace())
        }

        fn display_character_sheet(simulation: &mut Simulation, ui: &mut egui::Ui) {
            Frame::none().stroke(stroke(ui)).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Character Sheet").strong());
                });

                ui.vertical(|ui| {
                    make_frame(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Trait");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label("Value");
                            });
                        });

                        ui.separator();
                        for (k, v) in [
                            ("Name", make_label(&simulation.player.name)),
                            ("Race", make_label(&simulation.player.race.name)),
                            ("Class", make_label(&simulation.player.class.name)),
                            ("Level", make_label(&simulation.player.level.to_string())),
                        ] {
                            ui.horizontal(|ui| {
                                ui.monospace(k);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.add(v);
                                });
                            });
                        }
                    });

                    make_frame(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Stat");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label("Value");
                            });
                        });
                        ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .min_scrolled_height(32.0)
                            .id_source("stat_list")
                            .show(ui, |ui| {
                                for (stat, val) in simulation.player.stats.iter() {
                                    ui.horizontal(|ui| {
                                        ui.monospace(stat.as_str());
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                ui.add(make_label(&val.to_string()));
                                            },
                                        );
                                    });
                                }
                            });
                    });

                    ui.label("Experience");
                    Progress::from_bar(
                        simulation.player.exp_bar,
                        crate::progress::ProgressInfo::NextLevel {
                            exp: simulation.player.exp_bar.remaining() as _,
                        },
                    )
                    .display(ui);
                });
            });
        }

        fn display_spell_book(simulation: &mut Simulation, ui: &mut egui::Ui) {
            Frame::none().stroke(stroke(ui)).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Spell Book").strong());
                });
                // ui.separator();

                make_frame(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Spell");
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label("Level");
                        });
                    });
                    ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .min_scrolled_height(32.0)
                        .id_source("spell_list")
                        .show(ui, |ui| {
                            for (spell, level) in simulation.player.spell_book.iter() {
                                ui.horizontal(|ui| {
                                    ui.monospace(spell);
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.add(make_label(&Roman::from_i32(level)));
                                    });
                                });
                            }

                            // ui.allocate_space(ui.available_size_before_wrap());
                        });
                });
            });
        }

        fn display_equipment(simulation: &mut Simulation, ui: &mut egui::Ui) {
            Frame::none().stroke(stroke(ui)).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Equipment").strong());
                });

                make_frame(ui, |ui| {
                    ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .id_source("equipment_list")
                        .show(ui, |ui| {
                            for (equipment, name) in simulation.player.equipment.iter() {
                                ui.horizontal(|ui| {
                                    ui.monospace(equipment.as_str());
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.add(make_label(name));
                                    });
                                });
                            }
                        });
                });
            });
        }

        fn display_inventory(simulation: &mut Simulation, ui: &mut egui::Ui) {
            Frame::none().stroke(stroke(ui)).show(ui, |ui| {
                TopBottomPanel::bottom("encumbrance_bar")
                    .resizable(false)
                    .show_separator_line(false)
                    .frame(Frame::none())
                    .show_inside(ui, |ui| {
                        make_frame(ui, |ui| {
                            ui.label("Encumbrance");
                            Progress::from_bar(
                                simulation.player.inventory.encumbrance,
                                crate::progress::ProgressInfo::Cubits {
                                    min: simulation.player.inventory.encumbrance.pos as _,
                                    max: simulation.player.inventory.encumbrance.max as _,
                                },
                            )
                            .display(ui);
                        });
                    });

                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Inventory").strong());
                });

                make_frame(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Item");
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label("Qty");
                        });
                    });

                    ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .id_source("inventory_list")
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.monospace("Gold");
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.add(make_label(
                                        &simulation.player.inventory.gold().to_string(),
                                    ));
                                });
                            });

                            for (name, qty) in simulation.player.inventory.items() {
                                ui.horizontal(|ui| {
                                    ui.monospace(name);
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.add(make_label(&qty.to_string()));
                                    });
                                });
                            }

                            // ui.allocate_space(ui.available_size_before_wrap());
                        });
                });
            });
        }

        fn display_plot(simulation: &mut Simulation, ui: &mut egui::Ui) {
            Frame::none().stroke(stroke(ui)).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Plot Development").strong());
                    ui.separator();
                });

                ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .id_source("plot_list")
                    .show(ui, |ui| {
                        Frame::none()
                            .inner_margin(Margin::symmetric(4.0, 2.0))
                            .show(ui, |ui| {
                                for act in 0..simulation.player.quest_book.act() {
                                    ui.checkbox(&mut true, act_name(act));
                                }
                                ui.checkbox(
                                    &mut false,
                                    act_name(simulation.player.quest_book.act()),
                                );

                                Progress::from_bar(
                                    simulation.player.quest_book.plot,
                                    crate::progress::ProgressInfo::Complete,
                                )
                                .display(ui);
                            });
                    });
            });
        }

        fn display_quests(simulation: &mut Simulation, ui: &mut egui::Ui) {
            Frame::none().stroke(stroke(ui)).show(ui, |ui| {
                TopBottomPanel::bottom("quest_bar")
                    .resizable(false)
                    .show_separator_line(false)
                    .frame(Frame::none())
                    .show_inside(ui, |ui| {
                        Progress::from_bar(
                            simulation.player.quest_book.quest,
                            crate::progress::ProgressInfo::Complete,
                        )
                        .display(ui);
                    });

                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Quests").strong());
                    ui.separator();
                });

                ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .id_source("quest_list")
                    .show(ui, |ui| {
                        Frame::none()
                            .inner_margin(Margin::symmetric(4.0, 2.0))
                            .show(ui, |ui| {
                                for quest in simulation.player.quest_book.completed_quests() {
                                    ui.checkbox(&mut true, quest);
                                }

                                if let Some(quest) = simulation.player.quest_book.current_quest() {
                                    ui.checkbox(&mut false, quest);
                                }
                            });
                        ui.allocate_space(ui.available_size_before_wrap());
                    });
            });
        }

        simulation.tick(rng);

        CentralPanel::default().show(ctx, |ui| {
            // ui.horizontal(|ui| {
            //     ui.add(egui::Slider::new(&mut simulation.time_scale, 1.0..=100.0).step_by(5.0));
            // });

            simulation.time_scale = simulation.time_scale.max(1.0);

            TopBottomPanel::bottom("bottom_panel")
                .frame(Frame::none())
                .resizable(false)
                .show_separator_line(false)
                .show_inside(ui, |ui| {
                    ui.vertical(|ui| {
                        if let Some(task) = &simulation.player.task {
                            ui.label(&*task.description);
                        }
                        Progress::from_bar(
                            simulation.player.task_bar,
                            crate::progress::ProgressInfo::Percent,
                        )
                        .display(ui);
                        // ui.allocate_space(ui.available_size_before_wrap());
                    });
                });

            SidePanel::left("left_panel")
                .frame(Frame::none())
                .resizable(false)
                .show_separator_line(false)
                .show_inside(ui, |ui| {
                    display_character_sheet(simulation, ui);
                    display_spell_book(simulation, ui);
                });

            SidePanel::right("right_panel")
                .frame(Frame::none())
                .resizable(false)
                .show_separator_line(false)
                .show_inside(ui, |ui| {
                    display_plot(simulation, ui);
                    display_quests(simulation, ui);
                });

            display_equipment(simulation, ui);
            display_inventory(simulation, ui);
        });

        ctx.request_repaint_after(Self::FRAME_RATE);
    }

    fn display_main_view(view: &mut View, rng: &Rand, ctx: &egui::Context) {
        *view = match std::mem::take(view) {
            View::CharacterSelect { mut players } => {
                CentralPanel::default()
                    .show(ctx, |ui| {
                        use SelectionResult::*;
                        match Self::display_character_select(&mut players, ui) {
                            Selected(active) => View::run_simulation(active, players),
                            Details(active) => View::character_detail(active, players),
                            Create => {
                                let (player, stats_builder) = Self::make_new_character(rng);
                                View::character_creation(player, stats_builder, players)
                            }
                            Nothing => View::character_select(players),
                        }
                    })
                    .inner
            }

            View::CharacterDetail { active, players } => {
                CentralPanel::default()
                    .show(ctx, |ui| {
                        use DetailsResult::*;
                        match Self::display_character_detail(&players[active], ui) {
                            Play => View::run_simulation(active, players),
                            Close => View::character_select(players),
                            Nothing => View::character_detail(active, players),
                        }
                    })
                    .inner
            }

            View::CharacterCreation {
                mut player,
                mut stats_builder,
                mut players,
            } => {
                CentralPanel::default()
                    .show(ctx, |ui| {
                        use CreationResult::*;
                        let creation = Self::display_character_creation(
                            &mut player,
                            &mut stats_builder,
                            rng,
                            ui,
                        );
                        match creation {
                            Created => {
                                players.push(player);
                                View::run_simulation(players.len() - 1, players)
                            }
                            Cancel => View::character_select(players),
                            Nothing => View::character_creation(player, stats_builder, players),
                        }
                    })
                    .inner
            }

            View::RunSimulation {
                mut simulation,
                active,
                players,
            } => {
                Self::display_game(&mut simulation, rng, ctx);
                View::RunSimulation {
                    simulation,
                    active,
                    players,
                }
            }

            View::Empty => unreachable!("invalid state"),
        }
    }

    fn maybe_process_tray(&mut self, frame: &mut eframe::Frame) {
        if let Ok(TrayEvent {
            event: tray_icon::ClickEvent::Double,
            ..
        }) = tray_icon::TrayEvent::receiver().try_recv()
        {
            self.is_visible = !self.is_visible;
            frame.set_visible(self.is_visible)
        }
    }
}

impl eframe::App for MainWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        const DEBUG_KEY: egui::KeyboardShortcut =
            egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F12);
        if ctx.input_mut().consume_shortcut(&DEBUG_KEY) {
            ctx.set_debug_on_hover(!ctx.debug_on_hover())
        }
        egui::gui_zoom::zoom_with_keyboard_shortcuts(ctx, frame.info().native_pixels_per_point);

        self.maybe_process_tray(frame);
        Self::display_main_view(&mut self.view, &self.rng, ctx)
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some((players, active)) = self.view.players() {
            // this moves the active player to the first slot
            let players = active.into_iter().chain(players).collect::<Vec<_>>();
            eframe::set_value(storage, Self::SETTINGS_KEY, &players);
        }
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }
}
