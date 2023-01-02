pub struct Roman;
impl Roman {
    pub fn from_i32(mut number: i32) -> String {
        #[rustfmt::skip]
        const fn to_char(d: i32) -> char {
            match d {
                1000 => 'M', 100 => 'C', 10 => 'X',
                500  => 'D', 50  => 'L', 5  => 'V',
                1    => 'I',
                _ => unreachable!(),
            }
        }

        let mut numerals = String::new();

        for (k, v) in [
            (100, 1000),
            (100, 500),
            (10, 100),
            (10, 50),
            (1, 10),
            (1, 5),
        ] {
            while number >= v {
                number -= v;
                numerals.push(to_char(v));
            }

            let diff = v - k;
            if number >= diff {
                number -= diff;
                numerals.extend([to_char(k), to_char(v)]);
            }
        }

        numerals.extend((0..number).map(|_| 'I'));
        numerals
    }

    pub fn to_roman(input: &str) -> i32 {
        input
            .chars()
            .rev()
            .map(|c| match c {
                'M' => 1000,
                'D' => 500,
                'C' => 100,
                'L' => 50,
                'X' => 10,
                'V' => 5,
                'I' => 1,
                _ => 0,
            })
            .fold((0_i32, 0_i32), |(a, max), n| {
                (a + (n >= max).then_some(n).unwrap_or(-n), max.max(n))
            })
            .0
    }
}

#[test]
fn roman() {
    for (num, cmp) in [
        ("MMXIV", 2014),
        ("MCMXCIX", 1999),
        ("XXV", 25),
        ("MDCLXVI", 1666),
        ("MMMDCCCLXXXVIII", 3888),
    ] {
        assert_eq!(Roman::from_i32(cmp), num, "{num}");
        assert_eq!(Roman::to_roman(num), cmp, "{cmp}");
    }
}
