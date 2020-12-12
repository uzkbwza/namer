use fs::File;
use io::prelude::*;
use rand::prelude::*;

use serde_derive::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::fs;
use std::panic::catch_unwind;
use std::io;
use std::io::Write;
use std::sync::Arc;

#[derive(Clone)]
pub struct Markov {
    // size of n-grams
    pub ngram_size: usize,
    pub minimum_length: usize,
    pub regen_chance: usize,
    pub name: String,
    map: HashMap<String, Vec<Entry>>,
    corpus: Arc<Vec<String>>,
    starting_ngrams: Vec<Vec<char>>,
}

impl Markov {
    pub fn new(
        ngram_size: usize,
        mut minimum_length: usize,
        regen_chance: usize,
        name: String,
        corpus: Arc<Vec<String>>,
    ) -> Self {
        if minimum_length < ngram_size {
            minimum_length = ngram_size
        }
        let markov = Markov {
            ngram_size,
            minimum_length,
            regen_chance,
            name,
            map: HashMap::new(),
            starting_ngrams: Vec::new(),
            corpus,
        };
        markov.load()
    }

    pub fn generate(&self) -> String {
        //println!("generating");
        let mut valid = false;
        let mut result = String::from("");
        let mut rng = thread_rng();
        while !valid {
            let must_have_space: bool = rng.gen_bool(2.7 / 3.0);
            let force_capitalization: bool = rng.gen_bool(1.0 / 3.0);
            let force_case_homogeneity: bool = rng.gen_bool(1.5 / 3.0);
            let mut key: String = self
                .starting_ngrams
                .choose(&mut rng)
                .expect("No starting ngrams, have you initialized the map?")
                .clone()
                .iter()
                .collect();

            //        let mut key: String = " ".to_string();

            result = key.clone();

            loop {
                let value = self.get_value(key.clone(), &rng);
                match value {
                    Some(c) => {
                        result = format!("{}{}", result, &c);
                        if rng.gen::<usize>() % 100 < self.regen_chance {
                            key = self
                                .starting_ngrams
                                .choose(&mut rng)
                                .expect("No starting ngrams, have you initialized the map?")
                                .clone()
                                .iter()
                                .collect();
                        // println!("swapping");
                        } else {
                            key = self.next_key(&key, c);
                        };
                    }
                    None => break,
                }
            }

            let mut blacklisted = false;
            let blacklist = vec![
                "nigger", "niggy", "nigga", "kike", "faggot", "hitler", "spic", "kkk", "1488",
            ];

            // let num_words_generated = result
            //    .split(' ')
            //    .collect::<Vec<&str>>()
            //    .len();

            for entry in blacklist {
                if result
                    //.to_lowercase()
                    .contains(&entry)
                {
                    blacklisted = true;
                }
            }

            if result.len() < self.minimum_length
                // name is not in corpus nor is a substring of any
                // name in corpus. this is an EXTREMELY costly
                // operation, i'm not sure how to optimize.
                // || self.corpus
                //    .iter()
                //    .any(|s| s.contains(&result))
                // || (!result.contains(' ')
                //     && !result.contains('-'))
                || blacklisted
                || result.len() > 70
            //|| num_words_generated != num_words
            {
                //result.clear();
                //result = self.generate(num_words)
                valid = false;
                continue;
            }

            // if some are lowercase, make all lowercase
            // sometimes make all the first letters uppercase
            let mut chars: Vec<char> = result.chars().collect(); 
            let mut lowercase_beginning = false;
            let mut lowercase_beginnings = vec![0];
            let mut uppercase_beginnings= vec![0];
            let mut beginnings = vec![0];
            let mut endings = Vec::new();
            let mut open_braces = Vec::new();
            let mut open_chars = [' '; 32];
            let mut closed_braces = Vec::new();
            let mut closed_chars = [' '; 32];
            if chars[0].is_ascii_lowercase() {
                lowercase_beginning = true;
            }
            if force_capitalization {
                chars[0] = chars[0].to_ascii_uppercase();
            }
            chars.truncate(32);
            for i in 1..chars.len() {
                let c = chars[i - 1];
                let d = chars[i];
                if i < chars.len() - 1 {
                    let e = chars[i + 1];
                    if e == ' ' {
                        endings.push(i);
                    }
                } else {
                    endings.push(i)
                }
                if c == '(' || c == '[' || c == '{' {
                    open_braces.push(i - 1);
                    open_chars[i - 1] = c;
                }
                if d == ')' || c == ']' || c == '}' {
                    closed_braces.push(i);
                    closed_chars[i] = d;
                }
                if c == ' ' {
                    if force_capitalization {
                        chars[i] = d.to_ascii_uppercase();
                    }
                    if d.is_ascii_lowercase() {
                        lowercase_beginning = true;
                        lowercase_beginnings.push(i);
                        beginnings.push(i);
                    } else {
                        uppercase_beginnings.push(i);
                        beginnings.push(i);
                    } 
                }
            }

            if lowercase_beginning && force_case_homogeneity { 
                let num_upper = uppercase_beginnings.len() as f64;
                let num_lower = lowercase_beginnings.len() as f64;
                let p = (num_lower * 3.) / (num_upper + (num_lower * 3.));
                let force_lowercase_homogeneity: bool = rng
                    .gen_bool(p);
                if force_lowercase_homogeneity {
                    for i in uppercase_beginnings {
                        chars[i] = chars[i].to_ascii_lowercase();
                    }
                } else {
                    for i in lowercase_beginnings {
                        chars[i] = chars[i].to_ascii_uppercase();
                    }
                }
            }
            
            // match braces 
            while open_braces.len() > closed_braces.len() && !open_braces.is_empty() {
                let open_location = open_braces.remove(0);
                let open_char = open_chars[open_location];
                let closed_char = match open_char {
                    '(' => ')',
                    '[' => ']',
                    '{' => '}',
                    _ => '⁋' 
                };
                let insert_pos = *closed_braces.iter().filter(|x| **x > open_location).choose(&mut rng).unwrap_or(&(chars.len()));
                chars.insert(insert_pos, closed_char);
            }

            while open_braces.len() < closed_braces.len() && !closed_braces.is_empty() {
                let closed_location = closed_braces.remove(0);
                let closed_char = closed_chars[closed_location];
                let open_char = match closed_char {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => '⁋' 
                };
                let insert_pos = *open_braces.iter().filter(|x| **x < closed_location).choose(&mut rng).unwrap_or(&0);
                chars.insert(insert_pos, open_char);
            }

            let beginning = chars[0];
            //    .to_ascii_uppercase();

            let end: String = chars.split_off(1).into_iter().collect();

            result = format!("{}{}", beginning, end);

            result = result.trim().to_string();
            //if self.corpus.contains(&result)
            if (result.len() > 32)
                || (!result.contains(' ') && must_have_space)
            {
                valid = false;
            } else {
                valid = true;
            }
            // reroll if it's not unique
        }

        result
    }

    fn next_key(&self, key: &str, value: char) -> String {
        let mut last = key.to_string();
        last.remove(0);
        last.push(value);
        last
    }

    fn hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        Hash::hash_slice(&self.corpus, &mut hasher);
        hasher.finish()
    }

    fn build(mut self) -> Self {
        println!("building {}", &self.name);
        let corpus = match self.corpus.len() {
            0 => panic!("No corpus added!"),
            _ => self.corpus.clone(),
        };

        for string in corpus.iter() {
            // println!("Processed: {1}/{0}", corpus.len(), i);
            let chars: Vec<char> = string.chars().collect();

            if chars.len() < self.minimum_length {
                continue;
            }

            self.starting_ngrams
                .push(chars[0..self.ngram_size].to_vec());

            for i in 0..=chars.len() - self.ngram_size {
                let mut key = String::new();

                for j in 0..self.ngram_size {
                    key.push(chars[i + j]);
                }

                if i == chars.len() - self.ngram_size {
                    self.insert(key, None);
                    continue;
                }

                let value = chars[i + self.ngram_size];
                self.insert(key, Some(value));
            }
        }
        self.save();
        self
    }

    fn save(&self) {
        println!("saving");
        let mut file =
            &File::create(format!("data/{}_n{}", self.name, self.ngram_size.to_string())).unwrap();
        let data = FileData {
            corpus_hash: self.hash(),
            map: self.map.clone(),
            starting_ngrams: self.starting_ngrams.clone()
        };
        let encoded: Vec<u8> = bincode::serialize(&data).unwrap();
        file.write_all(&encoded).ok();
    }

    fn load(mut self) -> Self {
        println!("loading {}, n size {}...", self.name, self.ngram_size);
        // try to load file if it exists
        let check_file = File::open(format!("data/{}_n{}", self.name, self.ngram_size.to_string()));
        let mut input_file = match check_file {
            Ok(file) => {
                file
            },
            Err(_) => { 
                self = self.build();
                File::open(format!("data/{}_n{}", self.name, self.ngram_size.to_string())).unwrap()          
            }
        };
        let mut buffer = Vec::<u8>::new();
        input_file.read_to_end(&mut buffer).ok();
        let file_data: FileData = bincode::deserialize(&buffer).unwrap();
        if file_data.corpus_hash != self.hash() {
            self = self.build();
            return self.load();
        }
        self.map = file_data.map;
        self.starting_ngrams = file_data.starting_ngrams;
        self
    }

    fn get_value(&self, key: String, rng: &ThreadRng) -> Option<char> {
        catch_unwind (|| {
            let values = &self.map[&key];
            values
                .choose_weighted(&mut rng.clone(), |item| item.weight)
                .expect("Could not select value!")
                .ch
        }).ok().flatten()
    }

    fn insert(&mut self, key: String, value: Option<char>) {
        if let Some(k) = self.map.get_mut(&key) {
            for entry in k {
                if entry.ch == value {
                    entry.weight += 1;
                    return;
                }
            }
        } else {
            let new_entry = value.into();
            self.map.insert(key, vec![new_entry]);
            return;
        }

        if let Some(k) = self.map.get_mut(&key) {
            let new_entry = value.into();
            k.push(new_entry);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub ch: Option<char>,
    pub weight: u32,
}

impl Entry {
    pub fn new(ch: Option<char>, weight: u32) -> Self {
        Entry { ch, weight }
    }
}

impl From<char> for Entry {
    fn from(ch: char) -> Self {
        Entry::new(Some(ch), 1)
    }
}
impl From<Option<char>> for Entry {
    fn from(ch: Option<char>) -> Self {
        Entry::new(ch, 1)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileData {
    pub corpus_hash: u64,
    pub map: HashMap<String, Vec<Entry>>,
    pub starting_ngrams: Vec<Vec<char>>,
}
