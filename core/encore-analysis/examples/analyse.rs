//! Analyses audio files and prints one JSON line per file (for evaluating the fingerprint on
//! real music):
//!   cargo run --release -p encore-analysis --example analyse -- [--model yamnet-v1.bin] file1.mp3 ...
//! With `--model`, each line also carries YAMNet's 1024-number embedding and 521 class scores.

use std::fs::File;
use std::time::Instant;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let model = args.iter().position(|a| a == "--model").map(|i| {
        let path = args[i + 1].clone();
        args.drain(i..i + 2);
        let bytes = std::fs::read(&path).expect("the model file reads");
        encore_analysis::Yamnet::from_bytes(&bytes).expect("the model file loads")
    });
    for path in args {
        let started = Instant::now();
        let ext = std::path::Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_owned);
        let result = File::open(&path)
            .map_err(|e| encore_analysis::AnalysisError::Decode(e.to_string()))
            .and_then(|f| {
                encore_analysis::analyse_song(Box::new(f), ext.as_deref(), model.as_ref())
            });
        let ms = started.elapsed().as_millis();
        match result {
            Ok(a) => {
                let p = &a.profile;
                let texture: Vec<String> = p.texture.iter().map(|v| v.to_string()).collect();
                let key = p
                    .key
                    .map_or("null".to_string(), |k| format!("\"{}\"", k.name()));
                let numbers = |v: &[f32]| {
                    v.iter()
                        .map(|x| format!("{x:.4}"))
                        .collect::<Vec<_>>()
                        .join(",")
                };
                let (pooled, scores) = a
                    .embedding
                    .as_ref()
                    .map_or((String::new(), String::new()), |e| {
                        (numbers(&e.pooled), numbers(&e.scores))
                    });
                println!(
                    "{{\"file\":{path:?},\"ms\":{ms},\"bpm\":{:.1},\"beat\":{:.3},\"key\":{key},\"lufs\":{:.1},\"energy\":{:.3},\"brightness\":{:.3},\"dynamics\":{:.3},\"texture\":[{}],\"pooled\":[{pooled}],\"scores\":[{scores}]}}",
                    p.bpm,
                    p.beat_strength,
                    p.lufs,
                    p.energy,
                    p.brightness,
                    p.dynamics,
                    texture.join(",")
                );
            }
            Err(e) => println!(
                "{{\"file\":{path:?},\"ms\":{ms},\"error\":{:?}}}",
                e.to_string()
            ),
        }
    }
}
