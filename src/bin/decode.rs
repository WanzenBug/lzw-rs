extern crate lzw;

#[cfg(feature = "bin")]
extern crate clap;

#[cfg(feature = "bin")]
mod util;

#[cfg(feature = "bin")]
fn main() {
    use std::fs::File;
    use std::io::{copy, BufReader, BufWriter, Write};
    use lzw::LZWReader;

    let matches = clap::App::new("LZW decoder")
        .version("0.1")
        .author("Moritz Wanzenböck <moritz.wanzenboeck@gmail.com>")
        .about("Decompress files using pure LZW")
        .arg(clap::Arg::with_name("INPUT")
            .required(true)
            .help("Sets the input file to use")
            .index(1))
        .arg(clap::Arg::with_name("OUTPUT")
            .required(true)
            .help("Sets the output file to use")
            .index(2))
        .arg(clap::Arg::with_name("dict-size")
            .short("d")
            .long("dict-size")
            .help("Set the maximum dictionary size to the specified value in bytes")
            .takes_value(true))
        .arg(clap::Arg::with_name("verbose")
            .short("v")
            .help("Sets verbose output"))
        .get_matches();

    let infile = matches.value_of("INPUT").and_then(|file| File::open(file).ok()).expect("Input: No such file");
    let outfile = matches.value_of("OUTPUT").and_then(|file| File::create(file).ok()).expect("Output: Could not create file");
    let dictsize = matches.value_of("dict-size").and_then(|val| val.parse::<usize>().ok()).expect("Need integer value");

    let mut read = util::StatsReader::new(BufReader::new(infile));
    let mut write = util::StatsWriter::new(BufWriter::new(outfile));
    {
        let mut reader = LZWReader::with_dictsize(&mut read, dictsize);
        copy(&mut reader, &mut write).expect("Something went wrong while encoding");
    }
    if matches.is_present("verbose") {
        write.flush().expect("Flush failed");
        let written = write.get_stats().processed;
        let read = read.get_stats().processed;

        println!("Read:    {} bytes", read);
        println!("Written: {} bytes", written);
    }
}

#[cfg(not(feature = "bin"))]
fn main() {
    println!("Feature not enabled");
    std::process::exit(1);
}