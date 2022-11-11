use std::fmt;
use std::collections::VecDeque;

pub struct BrailleGraph {
    data: VecDeque<u8>,
    length: usize,
}

impl BrailleGraph {
    pub fn new(length: usize) -> Self {
        Self {
            data: VecDeque::from(vec![0; length]),
            length,
        }
    }

    pub fn update(&mut self, pct: u8) {
        if self.data.len() >= self.length {
            self.data.pop_front();
        }
        self.data.push_back(pct);
    }

    fn pct_thresholds(i: u8) -> u8 {
        if i > 80 {
            4
        } else if i > 60 {
            3
        } else if i > 40 {
            2
        } else if i > 20 {
            1
        } else {
            0
        }
    }
}

impl fmt::Display for BrailleGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.data.iter().peekable();
        while iter.peek().is_some() {
            let next = **iter.peek().unwrap();
            let curr = *iter.next().unwrap();

            let c = match (BrailleGraph::pct_thresholds(next), BrailleGraph::pct_thresholds(curr)) {
                (0, 0) => '\u{2800}', // '⠀'
                (0, 1) => '\u{2880}', // '⢀'
                (0, 2) => '\u{28A0}', // '⢠'
                (0, 3) => '\u{28B0}', // '⢰'
                (0, 4) => '\u{28B8}', // '⢸'
                (1, 0) => '\u{2840}', // '⡀'
                (1, 1) => '\u{28C0}', // '⣀'
                (1, 2) => '\u{28E0}', // '⣠'
                (1, 3) => '\u{28F0}', // '⣰'
                (1, 4) => '\u{28F8}', // '⣸'
                (2, 0) => '\u{2844}', // '⡄'
                (2, 1) => '\u{28C4}', // '⣄'
                (2, 2) => '\u{28E4}', // '⣤'
                (2, 3) => '\u{28F4}', // '⣴'
                (2, 4) => '\u{28FC}', // '⣼'
                (3, 0) => '\u{2846}', // '⡆'
                (3, 1) => '\u{28C6}', // '⣆'
                (3, 2) => '\u{28E6}', // '⣦'
                (3, 3) => '\u{28F6}', // '⣶'
                (3, 4) => '\u{28FE}', // '⣾'
                (4, 0) => '\u{2847}', // '⡇'
                (4, 1) => '\u{28C7}', // '⣇'
                (4, 2) => '\u{28E7}', // '⣧'
                (4, 3) => '\u{28F7}', // '⣷'
                (4, 4) => '\u{28FF}', // '⣿'
                _ => unreachable!("WHOAHOA!")
            };
            write!(f, "{}", c)?;
        }
        Ok(())
    }
}
