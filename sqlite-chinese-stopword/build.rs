use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;

static DEFAULT_STOPWORD: &str = include_str!("../data/stopword.txt");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data/stopword.txt");

    // 构建停词表
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("stopword_data.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());
    let mut stopword = phf_codegen::OrderedSet::new();
    for line in DEFAULT_STOPWORD.split("\n") {
        if line.is_empty() || line.starts_with("#") {
            continue;
        }
        stopword.entry(line.trim());
    }

    write!(
        &mut file,
        "pub static STOPWORD: phf::OrderedSet<&'static str> = {}",
        stopword.build()
    )
    .unwrap();
    writeln!(&mut file, ";").unwrap();
}
