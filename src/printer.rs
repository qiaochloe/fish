use crate::card::{Card, PrintCard, PrintCardSize, RawCard, Suit};
use crate::{Fish, Player};
use colored::Colorize;
use std::cell::RefCell;
use std::fmt::Debug;
use std::fmt::Write as FmtWrite;
use std::rc::Rc;

// Helper functions to convert RawCard to Suit, Rank, and Card
pub trait PrettyDisplay {
    fn to_pretty_string(&self) -> String;
}

#[derive(Debug)]
pub struct Printer {
    pub use_color: Rc<RefCell<bool>>,
}

impl Printer {
    pub fn to_pretty_string(&self, obj: &(impl Debug + PrettyDisplay)) -> String {
        if *self.use_color.borrow() {
            obj.to_pretty_string()
        } else {
            format!("{obj:?}")
        }
    }

    // Print utilities
    pub fn print_hand(&self, player: usize, g: &Fish) -> String {
        let mut players = g.players.borrow_mut();
        players[player].cards.sort();
        self.to_pretty_string(&players[player].cards)
    }

    pub fn print_player(&self, player: usize, g: &Fish) -> String {
        let players = g.players.borrow();
        self.to_pretty_string(&players[player])
    }

    pub fn print_constraints(&self, player: usize, g: &Fish) -> String {
        let players = g.players.borrow();
        let e = players[player].ref_engine();
        let mut output = String::new();

        writeln!(
            &mut output,
            "           {} {} {} {} {} {} {} {} {}",
            " LOW ♦".to_string().blue(),
            "HIGH ♦".to_string().blue(),
            " LOW ♣".to_string().green(),
            "HIGH ♣".to_string().green(),
            " LOW ♥".to_string().red(),
            "HIGH ♥".to_string().red(),
            " LOW ♠".to_string().bright_black(),
            "HIGH ♠".to_string().bright_black(),
            "EIGHT ".to_string().bright_black(),
        )
        .unwrap();

        for (player, bits) in e.to_matrix().iter() {
            let bits_str: String = bits
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    if *b {
                        PrintCard {
                            card: RawCard { num: i as u8 }.as_card(),
                            size: PrintCardSize::Short,
                        }
                        .to_pretty_string()
                    } else {
                        ".".to_string()
                    }
                })
                .collect::<Vec<String>>()
                .chunks(6)
                .map(|chunk| chunk.join(""))
                .collect::<Vec<String>>()
                .join("|");

            writeln!(
                &mut output,
                "[{}] {}",
                self.print_player(*player, g),
                bits_str
            )
            .unwrap();
        }
        output
    }
}

impl PrettyDisplay for PrintCard {
    fn to_pretty_string(&self) -> String {
        let str_func = {
            match self.size {
                PrintCardSize::Short => Card::to_short_string,
                PrintCardSize::Full => Card::to_string,
            }
        };
        match self.card {
            Card::Standard { ref suit, .. } => match suit {
                Suit::Diamonds => str_func(&self.card).blue(),
                Suit::Clubs => str_func(&self.card).green(),
                Suit::Hearts => str_func(&self.card).red(),
                Suit::Spades => str_func(&self.card).bright_black(),
            },
            Card::Joker { big } => {
                if big {
                    str_func(&self.card).blue()
                } else {
                    str_func(&self.card).red()
                }
            }
        }
        .to_string()
    }
}

impl PrettyDisplay for RawCard {
    fn to_pretty_string(&self) -> String {
        PrintCard {
            card: self.as_card(),
            size: PrintCardSize::Full,
        }
        .to_pretty_string()
    }
}

impl PrettyDisplay for Player {
    fn to_pretty_string(&self) -> String {
        if self.idx % 2 == 0 {
            format!("{}", format!("Player {}", self.idx).blue())
        } else {
            format!("{}", format!("Player {}", self.idx).red())
        }
    }
}

impl<T: PrettyDisplay> PrettyDisplay for Vec<T> {
    fn to_pretty_string(&self) -> String {
        format!(
            "[{}]",
            self.iter()
                .map(|item| item.to_pretty_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
