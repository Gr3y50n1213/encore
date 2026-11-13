//! Analyses audio files and prints one JSON line per file (for evaluating the fingerprint on
//! real music): `cargo run --release -p encore-analysis --example analyse -- file1.mp3 file2.mp3`

use std::fs::File;
use std::time::Instant;

fn main() {
    for path in std::env::args().skip(1) {
        let started = Instant::now();
        let ext = std::path::Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_owned);
        let result = File::open(&path)
            .map_err(|e| encore_analysis::AnalysisError::Decode(e.to_string()))
            .and_then(|f| encore_analysis::analyse_source(Box::new(f), ext.as_deref()));
        let ms = started.elapsed().as_millis();
        match result {
            Ok(p) => {
                let texture: Vec<String> = p.texture.iter().map(|v| v.to_string()).collect();
                let key = p
                    .key
                    .map_or("null".to_string(), |k| format!("\"{}\"", k.name()));
                println!(
                    "{{\"file\":{path:?},\"ms\":{ms},\"bpm\":{:.1},\"beat\":{:.3},\"key\":{key},\"lufs\":{:.1},\"energy\":{:.3},\"brightness\":{:.3},\"dynamics\":{:.3},\"texture\":[{}]}}",
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
