extern crate atomic_counter;
extern crate bincode;
extern crate confy;
extern crate ctrlc;
extern crate rand;
extern crate serde;
extern crate time;

#[macro_use]
extern crate serde_derive;

mod markov;
use atomic_counter::*;
use fs::File;
use io::BufRead;
use io::BufWriter;
use markov::*;
use rand::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;
use time::PreciseTime;
use rayon::prelude::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Config {
    inputs: Vec<(usize, String, Vec<String>)>,
    n_gram_size: usize,
    min_length: usize,
    tweet: bool,
    regen_chance: usize,
    count: usize,
    threads: usize,
    print_every: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            inputs: vec![
                (
                    8,
                    "presidents_and_lovecraft".to_string(),
                    vec![
                        "presidents.txt".to_string(), 
                        "lovecraft.txt".to_string()],
                    )
            ],
            n_gram_size: 0,
            min_length: 0,
            tweet: false,
            regen_chance: 5,
            count: 100,
            threads: 1,
            print_every: 5,
        }
    }
}

fn get_models(
    n_gram_sizes: &[(usize, usize)],
    corpuses: &HashMap<String, Arc<Vec<String>>>,
    cfg: &Config,
) -> Vec<(usize, Vec<(usize, Markov)>)> {
    let mut models = Vec::new();
    //println!("{:#?}", corpuses);
    for input in &cfg.inputs {
        let mut sized_models = Vec::new();
        for (weight, size) in n_gram_sizes {
            let markov = Markov::new(*size, cfg.min_length, cfg.regen_chance, input.1.clone(), corpuses[&input.1].clone());
            sized_models.push((*weight, markov));
        }
        models.push((input.0, sized_models));
    }
    models
}

fn main() {
    let cfg: Config = confy::load_path("namer.toml").unwrap();
    let mut n_gram_sizes = Vec::new();

    if cfg.n_gram_size == 0 {
        n_gram_sizes.append(&mut vec![(1, 1), (4, 2), (2, 3)])
    } else {
        n_gram_sizes = vec![(1, cfg.n_gram_size)];
    };

    let mut corpuses = HashMap::new();
    for input in &cfg.inputs {
        let mut corpus = Vec::new();
        for file in &input.2 {
            println!("{}: adding input {:?}", &input.1, &file);
            corpus.append(&mut open_file(&file).unwrap())
        }
        corpuses.insert(input.1.clone(), Arc::new(corpus));
    }

    let models = Arc::new(get_models(&n_gram_sizes, &corpuses, &cfg));

    println!("ngram size {}", cfg.n_gram_size);

    let names = Arc::new(Mutex::new(HashSet::new()));
    let output = File::create("output.txt").unwrap();
    let mut writer = BufWriter::new(&output);

    let threads = {
        if cfg.count < cfg.threads {
            cfg.count
        } else {
            cfg.threads
        }
    };

    let start = PreciseTime::now();

    let width = models
        .par_iter()
        .max_by_key(|item| item.1[0].1.name.len())
        .unwrap()
        .1[0]
        .1
        .name
        .len();
    let numwidth = cfg.count.to_string().len();

    let counter = Arc::new(RelaxedCounter::new(0));
    let mut handles = Vec::new();
    for thread in 0..threads {
        let cfg = cfg.clone();
        let shared_models = models.clone();
        let i = counter.clone();
        let thread_names = names.clone();
        handles.push(thread::spawn(move || {
            while i.get() < cfg.count {
                let mut rng = thread_rng();

                let markov = &shared_models
                    .choose_weighted(&mut rng, |item| item.0)
                    .unwrap()
                    .1
                    .choose_weighted(&mut rng, |item| item.0)
                    .unwrap()
                    .1;
                let name = markov.generate(); 
                let mut names = thread_names.lock().unwrap();
                if i.get() < cfg.count && names.insert(name.clone()) {
                    if i.get() % cfg.print_every == 0 {
                        println!(
                        "thread: {:>3}, #{:<numwidth$}| {:>width$} | n: {}, name: {:<32}",
                        thread,
                        i.get(),
                        markov.name,
                        markov.ngram_size,
                        name,
                        width = width,
                        numwidth = numwidth
                    );
                    }
                    i.inc();
                }
            }
        }));
    }

    for handle in handles {
        handle.join().ok();
    }

    // let mut names = handles
    //  .drain(..)
    //  .filter_map(|x|
    //              x
    //               .join()
    //               .ok()
    //  )
    //  .flatten()
    //  .collect::<HashSet<String>>();

    let end = PreciseTime::now();
    println!(
        "{} seconds for generation and vector-building.",
        start.to(end)
    );
    println!("Writing to output.txt...");
    let names = names.lock().unwrap();
    for name in names.iter() {
        writeln!(&mut writer, "{}", name).ok();
    }
    println!("Done.")
}

fn open_file(pathname: &str) -> Result<Vec<String>, io::Error> {
    use io::BufReader;

    let input_file = File::open(pathname)?;
    let reader = BufReader::new(input_file);
    let output: Result<Vec<_>, _> = reader.lines().collect();
    //println!("output: {:#?}", &output);
    output
}
