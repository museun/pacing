use crate::mechanics::{Player, Simulation, StatsBuilder};

#[derive(Default)]
pub enum View {
    CharacterSelect {
        players: Vec<Player>,
    },
    CharacterDetail {
        active: usize,
        players: Vec<Player>,
    },
    CharacterCreation {
        player: Player,
        stats_builder: StatsBuilder,
        players: Vec<Player>,
    },
    RunSimulation {
        simulation: Simulation,
        active: usize,
        players: Vec<Player>,
    },
    #[default]
    Empty,
}

impl View {
    pub const fn character_select(players: Vec<Player>) -> Self {
        Self::CharacterSelect { players }
    }

    pub const fn character_detail(active: usize, players: Vec<Player>) -> Self {
        Self::CharacterDetail { active, players }
    }

    pub const fn character_creation(
        player: Player,
        stats_builder: StatsBuilder,
        players: Vec<Player>,
    ) -> Self {
        Self::CharacterCreation {
            player,
            stats_builder,
            players,
        }
    }

    pub fn run_simulation(active: usize, mut players: Vec<Player>) -> Self {
        let player = players.remove(active);

        Self::RunSimulation {
            active,
            players,
            simulation: Simulation::new(player),
        }
    }

    pub fn players(&self) -> Option<(&[Player], Option<&Player>)> {
        match self {
            Self::CharacterSelect { players }
            | Self::CharacterCreation { players, .. }
            | Self::CharacterDetail { players, .. } => Some((players, None)),
            Self::RunSimulation {
                players,
                simulation,
                ..
            } => Some((players, Some(&simulation.player))),
            Self::Empty => None,
        }
    }
}
