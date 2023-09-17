use std::fs::File;
use std::io::{BufRead, BufReader};

use camino::Utf8Path as Path;
use camino::Utf8PathBuf as PathBuf;
use clap::{Parser, Subcommand};
use log::info;

use sourmash::collection::Collection;
use sourmash::index::revindex::{prepare_query, RevIndex, RevIndexOps};
use sourmash::prelude::*;
use sourmash::signature::{Signature, SigsTrait};

fn read_paths<P: AsRef<Path>>(paths_file: P) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let paths = BufReader::new(File::open(paths_file.as_ref())?);
    Ok(paths
        .lines()
        .map(|line| {
            let mut path = PathBuf::new();
            path.push(line.unwrap());
            path
        })
        .collect())
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Index {
        /// List of signatures to search
        siglist: PathBuf,

        /// ksize
        #[clap(short, long, default_value = "31")]
        ksize: u8,

        /// scaled
        #[clap(short, long, default_value = "1000")]
        scaled: usize,

        /// The path for output
        #[clap(short, long)]
        output: PathBuf,

        /// Index using colors
        #[clap(long = "colors")]
        colors: bool,
    },
    Update {
        /// List of signatures to search
        siglist: PathBuf,

        /// ksize
        #[clap(short, long, default_value = "31")]
        ksize: u8,

        /// scaled
        #[clap(short, long, default_value = "1000")]
        scaled: usize,

        /// The path for output
        #[clap(short, long)]
        output: PathBuf,
    },
    /* TODO: need the repair_cf variant, not available in rocksdb-rust yet
        Repair {
            /// The path for DB to repair
            #[clap(parse(from_os_str))]
            index: PathBuf,

            /// Repair using colors
            #[clap(long = "colors")]
            colors: bool,
        },
    */
    Check {
        /// The path for output
        output: PathBuf,

        /// avoid deserializing data, and without stats
        #[clap(long = "quick")]
        quick: bool,
    },
    Convert {
        /// The path for the input DB
        input: PathBuf,

        /// The path for the output DB
        output: PathBuf,
    },
    Search {
        /// Query signature
        query_path: PathBuf,

        /// Path to rocksdb index dir
        index: PathBuf,

        /// ksize
        #[clap(short = 'k', long = "ksize", default_value = "31")]
        ksize: u8,

        /// scaled
        #[clap(short = 's', long = "scaled", default_value = "1000")]
        scaled: usize,

        /// threshold_bp
        #[clap(short = 't', long = "threshold_bp", default_value = "50000")]
        threshold_bp: usize,

        /// minimum containment to report
        #[clap(short = 'c', long = "containment", default_value = "0.2")]
        containment: f64,

        /// The path for output
        #[clap(short = 'o', long = "output")]
        output: Option<PathBuf>,
    },
    Gather {
        /// Query signature
        query_path: PathBuf,

        /// Path to rocksdb index dir
        index: PathBuf,

        /// ksize
        #[clap(short = 'k', long = "ksize", default_value = "31")]
        ksize: u8,

        /// scaled
        #[clap(short = 's', long = "scaled", default_value = "1000")]
        scaled: usize,

        /// threshold_bp
        #[clap(short = 't', long = "threshold_bp", default_value = "50000")]
        threshold_bp: usize,

        /// The path for output
        #[clap(short = 'o', long = "output")]
        output: Option<PathBuf>,
    },
}

fn gather<P: AsRef<Path>>(
    queries_file: P,
    index: P,
    selection: Selection,
    threshold_bp: usize,
    _output: Option<P>,
) -> Result<(), Box<dyn std::error::Error>> {
    let query_sig = Signature::from_path(queries_file.as_ref())?
        .swap_remove(0)
        .select(&selection)?;

    let mut query = None;
    if let Some(q) = prepare_query(query_sig, &selection) {
        query = Some(q);
    }
    let query = query.expect("Couldn't find a compatible MinHash");

    let threshold = threshold_bp / query.scaled() as usize;

    let db = RevIndex::open(index.as_ref(), true)?;
    info!("Loaded DB");

    info!("Building counter");
    let (counter, query_colors, hash_to_color) = db.prepare_gather_counters(&query);
    // TODO: truncate on threshold?
    info!("Counter built");

    let matches = db.gather(
        counter,
        query_colors,
        hash_to_color,
        threshold,
        &query,
        Some(selection),
    )?;

    info!("matches: {}", matches.len());
    for match_ in matches {
        println!(
            "{} {} {}",
            match_.name(),
            match_.intersect_bp(),
            match_.f_match()
        )
    }

    Ok(())
}

fn search<P: AsRef<Path>>(
    queries_file: P,
    index: P,
    selection: Selection,
    threshold_bp: usize,
    minimum_containment: f64,
    _output: Option<P>,
) -> Result<(), Box<dyn std::error::Error>> {
    let query_sig = Signature::from_path(queries_file.as_ref())?
        .swap_remove(0)
        .select(&selection)?;

    let mut query = None;
    if let Some(q) = prepare_query(query_sig, &selection) {
        query = Some(q);
    }
    let query = query.expect("Couldn't find a compatible MinHash");
    let query_size = query.size() as f64;

    let threshold = threshold_bp / query.scaled() as usize;

    let db = RevIndex::open(index.as_ref(), true)?;
    info!("Loaded DB");

    info!("Building counter");
    let counter = db.counter_for_query(&query);
    info!("Counter built");

    let matches = db.matches_from_counter(counter, threshold);

    //info!("matches: {}", matches.len());
    println!("SRA ID,containment");
    matches
        .into_iter()
        .filter_map(|(path, size)| {
            let containment = size as f64 / query_size;
            if containment >= minimum_containment {
                println!(
                    "{},{}",
                    path.split("/").last().unwrap().split(".").next().unwrap(),
                    containment
                );
                Some(())
            } else {
                None
            }
        })
        .count();

    Ok(())
}

fn index<P: AsRef<Path>>(
    siglist: P,
    selection: Selection,
    output: P,
    colors: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Loading siglist");
    let index_sigs = read_paths(siglist)?;
    info!("Loaded {} sig paths in siglist", index_sigs.len());

    let collection = Collection::from_paths(&index_sigs)?.select(&selection)?;
    RevIndex::create(output.as_ref(), collection.try_into()?, colors)?;

    Ok(())
}

fn update<P: AsRef<Path>>(
    siglist: P,
    selection: Selection,
    output: P,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Loading siglist");
    let index_sigs = read_paths(siglist)?;
    info!("Loaded {} sig paths in siglist", index_sigs.len());

    let collection = Collection::from_paths(&index_sigs)?.select(&selection)?;

    let db = RevIndex::open(output.as_ref(), false)?;
    db.update(collection.try_into()?)?;

    Ok(())
}

fn convert<P: AsRef<Path>>(_input: P, _output: P) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
    /*
    info!("Opening input DB");
    let db = RevIndex::open(input.as_ref(), true);

    info!("Creating output DB");
    let output_db = RevIndex::create(output.as_ref(), true);

    info!("Converting input DB");
    db.convert(output_db)?;

    info!("Finished conversion");
    Ok(())
    */
}

fn check<P: AsRef<Path>>(output: P, quick: bool) -> Result<(), Box<dyn std::error::Error>> {
    info!("Opening DB");
    let db = RevIndex::open(output.as_ref(), true)?;

    info!("Starting check");
    db.check(quick);

    info!("Finished check");
    Ok(())
}

/* TODO: need the repair_cf variant, not available in rocksdb-rust yet
fn repair<P: AsRef<Path>>(output: P, colors: bool) {
    info!("Starting repair");
    RevIndex::repair(output.as_ref(), colors);
    info!("Finished repair");
}
*/

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    use Commands::*;

    let opts = Cli::parse();

    match opts.command {
        Index {
            output,
            siglist,
            ksize,
            scaled,
            colors,
        } => {
            let selection = Selection::builder()
                .ksize(ksize.into())
                .scaled(scaled as u32)
                .build();

            index(siglist, selection, output, colors)?
        }
        Update {
            output,
            siglist,
            ksize,
            scaled,
        } => {
            let selection = Selection::builder()
                .ksize(ksize.into())
                .scaled(scaled as u32)
                .build();

            update(siglist, selection, output)?
        }
        Check { output, quick } => check(output, quick)?,
        Convert { input, output } => convert(input, output)?,
        Search {
            query_path,
            output,
            index,
            threshold_bp,
            ksize,
            scaled,
            containment,
        } => {
            let selection = Selection::builder()
                .ksize(ksize.into())
                .scaled(scaled as u32)
                .build();

            search(
                query_path,
                index,
                selection,
                threshold_bp,
                containment,
                output,
            )?
        }
        Gather {
            query_path,
            output,
            index,
            threshold_bp,
            ksize,
            scaled,
        } => {
            let selection = Selection::builder()
                .ksize(ksize.into())
                .scaled(scaled as u32)
                .build();

            gather(query_path, index, selection, threshold_bp, output)?
        } /* TODO: need the repair_cf variant, not available in rocksdb-rust yet
                  Repair { index, colors } => repair(index, colors),
          */
    };

    Ok(())
}
