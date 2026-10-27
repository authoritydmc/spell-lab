//! QWERTY-aware substitution costs. These are used for confidence scoring,
//! never for the discrete edit distance exposed in the API.

/// A cheap QWERTY keyboard-neighbour test. We treat horizontal, vertical and
/// diagonal neighbours as "near" keys.
pub fn substitution_cost(a: char, b: char) -> f64 {
    if a == b {
        return 0.0;
    }
    let a = a.to_ascii_lowercase();
    let b = b.to_ascii_lowercase();
    if !a.is_ascii_alphabetic() || !b.is_ascii_alphabetic() {
        return 1.0;
    }
    match (position(a), position(b)) {
        (Some((ax, ay)), Some((bx, by))) => {
            let dx = (ax - bx).abs();
            let dy = (ay - by).abs();
            if dx <= 1 && dy <= 1 {
                0.55
            } else if dx == 2 && dy == 1 {
                0.8
            } else {
                1.0
            }
        }
        _ => 1.0,
    }
}

pub fn is_keyboard_neighbour(a: char, b: char) -> bool {
    substitution_cost(a, b) < 0.7
}

fn position(c: char) -> Option<(i32, i32)> {
    let row = match c {
        'q' | 'w' | 'e' | 'r' | 't' | 'y' | 'u' | 'i' | 'o' | 'p' => 0,
        'a' | 's' | 'd' | 'f' | 'g' | 'h' | 'j' | 'k' | 'l' => 1,
        'z' | 'x' | 'c' | 'v' | 'b' | 'n' | 'm' => 2,
        _ => return None,
    };
    let offset = match row {
        0 => 0,
        1 => 0,
        _ => 1,
    };
    let col = match c {
        'q' => 0,
        'w' => 1,
        'e' => 2,
        'r' => 3,
        't' => 4,
        'y' => 5,
        'u' => 6,
        'i' => 7,
        'o' => 8,
        'p' => 9,
        'a' => 0,
        's' => 1,
        'd' => 2,
        'f' => 3,
        'g' => 4,
        'h' => 5,
        'j' => 6,
        'k' => 7,
        'l' => 8,
        'z' => 0,
        'x' => 1,
        'c' => 2,
        'v' => 3,
        'b' => 4,
        'n' => 5,
        'm' => 6,
        _ => return None,
    };
    Some((col + offset, row))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_neighbours() {
        assert!(is_keyboard_neighbour('a', 's'));
        assert!(is_keyboard_neighbour('q', 'w'));
        assert!(is_keyboard_neighbour('e', 'd'));
        assert!(!is_keyboard_neighbour('a', 'p'));
        assert_eq!(substitution_cost('x', 'x'), 0.0);
    }
}
