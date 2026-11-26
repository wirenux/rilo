// Single-line comment

/*
 * Multi-line block comment
 */

pub struct SampleConfig {
    pub count: usize,
    pub ratio: f64,
    pub active: bool,
}

impl SampleConfig {
    pub fn new(count: usize, ratio: f64, active: bool) -> Self {
        Self {
            count,
            ratio,
            active,
        }
    }
}

fn classify(n: i32) -> &'static str {
    if n < 0 {
        "negative"
    } else if n == 0 {
        "zero"
    } else {
        "positive"
    }
}

fn main() {
    let hex: u32 = 0xff;
    let dec: i64 = 12345;
    let float_val: f32 = 3.14;
    let ch: char = 'r';
    let escaped: &str = "hello \"rust\" \n world";
    let mut total: usize = 0;

    for i in 0..10 {
        if i == 7 {
            break;
        }
        if i % 2 == 0 {
            continue;
        }
        total += i;
    }

    let label = classify(dec as i32);
    let cfg = SampleConfig::new(total, float_val as f64, true);

    println!(
        "{label} | hex={hex} | ch={ch} | escaped={escaped} | active={}",
        cfg.active
    );
}
