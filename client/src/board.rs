use anyhow::{anyhow, ensure};
use itertools::Itertools;

const COL_SIZE: usize = 6;
const ROW_SIZE: usize = 7;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Color {
    Yellow,
    Red,
}

type Slot = Option<Color>;

#[derive(Clone)]
pub struct Board {
    slots: [Slot; ROW_SIZE * COL_SIZE],
}

impl Board {
    pub fn new() -> Self {
        Self {
            slots: [None; ROW_SIZE * COL_SIZE],
        }
    }

    pub fn rows(&self) -> impl Iterator<Item = &[Slot]> {
        self.slots.chunks(ROW_SIZE)
    }

    pub fn play_turn(&mut self, col: usize, color: Color) -> anyhow::Result<()> {
        ensure!(col < ROW_SIZE);
        for row in (0..COL_SIZE).rev() {
            let slot = &mut self.slots[row * ROW_SIZE + col];
            if slot.is_none() {
                *slot = Some(color);
                return Ok(());
            }
        }

        Err(anyhow!("invalid move"))
    }
    pub fn get_winner(&self) -> Option<Color> {
        let rows = (0..COL_SIZE).map(|i| find_four_consecutive(self.row(i)));
        let columns = (0..ROW_SIZE).map(|i| find_four_consecutive(self.column(i)));
        let diagonals = (0..(ROW_SIZE + COL_SIZE - 1)).flat_map(|i| {
            [
                find_four_consecutive(self.left_diagonal(i)),
                find_four_consecutive(self.right_diagonal(i)),
            ]
        });
        rows.chain(columns)
            .chain(diagonals)
            .find_map(Option::flatten)
    }

    pub fn is_full(&self) -> bool {
        self.slots.iter().take(ROW_SIZE).all(|slot| slot.is_some())
    }

    fn get_slot(&self, i: usize, j: usize) -> Option<Slot> {
        if i >= COL_SIZE || j >= ROW_SIZE {
            return None;
        }
        self.slots.get(i * ROW_SIZE + j).cloned()
    }

    fn row(&self, i: usize) -> impl Iterator<Item = Slot> {
        self.slots[i * ROW_SIZE..(i + 1) * ROW_SIZE].iter().copied()
    }

    fn column(&self, i: usize) -> impl Iterator<Item = Slot> {
        self.slots.iter().skip(i).step_by(ROW_SIZE).copied()
    }

    //diagonal going from bottom left to top right
    fn right_diagonal(&self, num: usize) -> impl Iterator<Item = Slot> {
        (0..=num).filter_map(move |j| self.get_slot(num - j, j))
    }

    fn left_diagonal(&self, num: usize) -> impl Iterator<Item = Slot> {
        (0..=num).filter_map(move |i| {
            let j = (ROW_SIZE + i).checked_sub(num + 1)?;
            self.get_slot(i, j)
        })
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

fn find_four_consecutive<T, I>(iter: I) -> Option<T>
where
    T: PartialEq + Clone,
    I: IntoIterator<Item = T>,
{
    iter.into_iter().tuple_windows().find_map(|(a, b, c, d)| {
        if a == b && b == c && c == d {
            Some(a)
        } else {
            None
        }
    })
}

