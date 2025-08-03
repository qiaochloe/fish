use crate::card::{Card, DisplayCard, Suit};
use crate::engine::Engine;
use crate::{Fish, Player};
use colored::Colorize;
use std::cell::RefCell;
use std::fmt::Debug;
use std::fmt::Write as FmtWrite;
use std::rc::Rc;

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

    // Printers
    pub fn print_hand(&self, player: usize, g: &Fish) -> String {
        let mut players = g.players.borrow_mut();
        players[player].cards.sort();
        self.to_pretty_string(&players[player].cards)
    }

    pub fn print_player(&self, player: usize, g: &Fish) -> String {
        let players = g.players.borrow();
        self.to_pretty_string(&players[player])
    }

    pub fn print_constraints(&self, e: &Engine, g: &Fish) -> String {
        let mut output = String::new();
        for (player, bits) in e.to_matrix().iter() {
            let bits_str: String = bits
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    if *b {
                        (Card { num: i as u8 })
                            .display_card()
                            .to_short_string()
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

impl PrettyDisplay for Card {
    fn to_pretty_string(&self) -> String {
        match self.display_card() {
            DisplayCard::Joker { big } => {
                if big {
                    self.to_string().blue()
                } else {
                    self.to_string().red()
                }
            }
            DisplayCard::Standard { suit, .. } => match suit {
                Suit::Diamonds => self.to_string().blue(),
                Suit::Clubs => self.to_string().green(),
                Suit::Hearts => self.to_string().red(),
                Suit::Spades => self.to_string().bright_black(),
            },
        }
        .to_string()
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

impl PrettyDisplay for Player {
    fn to_pretty_string(&self) -> String {
        if self.idx % 2 == 0 {
            format!("{}", format!("Player {}", self.idx).blue())
        } else {
            format!("{}", format!("Player {}", self.idx).red())
        }
    }
}
