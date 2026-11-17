use std::collections::HashMap;
use std::fs;

use sherpa_onnx::{KeywordSpotter, KeywordSpotterConfig, OnlineStream};

use super::super::Issue;
use super::capture::SAMPLE_RATE;
use super::models::{Failure, WakeFiles};

const WORD_START: char = '\u{2581}';
const SCORE: f32 = 1.5;
const THRESHOLD: f32 = 0.25;

pub struct Spotter {
    phrase: String,
    kws: KeywordSpotter,
}

pub struct Listener<'a> {
    spotter: &'a Spotter,
    stream: OnlineStream,
}

impl Spotter {
    pub fn load(files: &WakeFiles, phrase: &str) -> Result<Self, Failure> {
        if !crate::platform::speech_engine_loads() {
            return Err((
                Issue::Failed,
                "the wake word engine library is missing next to the app".to_owned(),
            ));
        }
        let tokens = fs::read_to_string(&files.tokens)
            .map_err(|error| (Issue::Failed, error.to_string()))?;
        let keywords = Vocabulary::parse(&tokens).keywords(phrase).ok_or_else(|| {
            (
                Issue::Phrase,
                format!("\"{phrase}\" cannot be spotted: use English words"),
            )
        })?;
        let mut config = KeywordSpotterConfig::default();
        config.model_config.transducer.encoder = Some(path(&files.encoder));
        config.model_config.transducer.decoder = Some(path(&files.decoder));
        config.model_config.transducer.joiner = Some(path(&files.joiner));
        config.model_config.tokens = Some(path(&files.tokens));
        config.model_config.num_threads = 1;
        config.keywords_score = SCORE;
        config.keywords_threshold = THRESHOLD;
        config.keywords_buf = Some(format!("{keywords}\n"));
        let kws = KeywordSpotter::create(&config).ok_or_else(|| {
            (
                Issue::Failed,
                "could not load the wake word model".to_owned(),
            )
        })?;
        Ok(Self {
            phrase: phrase.to_owned(),
            kws,
        })
    }

    pub fn phrase(&self) -> &str {
        &self.phrase
    }

    pub fn listener(&self) -> Listener<'_> {
        Listener {
            spotter: self,
            stream: self.kws.create_stream(),
        }
    }
}

impl Listener<'_> {
    pub fn heard(&self, samples: &[f32]) -> bool {
        let kws = &self.spotter.kws;
        self.stream.accept_waveform(SAMPLE_RATE as i32, samples);
        while kws.is_ready(&self.stream) {
            kws.decode(&self.stream);
            if kws
                .get_result(&self.stream)
                .is_some_and(|result| !result.keyword.is_empty())
            {
                kws.reset(&self.stream);
                return true;
            }
        }
        false
    }
}

fn path(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

struct Vocabulary {
    ids: HashMap<String, u32>,
}

impl Vocabulary {
    fn parse(text: &str) -> Self {
        let ids = text
            .lines()
            .filter_map(|line| {
                let (token, id) = line.trim_end().rsplit_once(' ')?;
                Some((token.to_owned(), id.parse().ok()?))
            })
            .collect();
        Self { ids }
    }

    fn keywords(&self, phrase: &str) -> Option<String> {
        let words: Vec<String> = phrase
            .split_whitespace()
            .map(|word| {
                word.chars()
                    .filter(|c| c.is_alphanumeric() || *c == '\'')
                    .flat_map(char::to_uppercase)
                    .collect::<String>()
            })
            .filter(|word| !word.is_empty())
            .collect();
        if words.is_empty() {
            return None;
        }
        let mut tokens = Vec::new();
        for word in words {
            tokens.extend(self.encode(&format!("{WORD_START}{word}"))?);
        }
        Some(tokens.join(" "))
    }

    fn encode(&self, word: &str) -> Option<Vec<String>> {
        let mut parts: Vec<String> = word.chars().map(String::from).collect();
        loop {
            let best = parts
                .windows(2)
                .enumerate()
                .filter_map(|(i, pair)| {
                    self.ids
                        .get(&format!("{}{}", pair[0], pair[1]))
                        .map(|&id| (id, i))
                })
                .min();
            let Some((_, i)) = best else {
                break;
            };
            let next = parts.remove(i + 1);
            parts[i].push_str(&next);
        }
        parts
            .iter()
            .all(|part| self.ids.contains_key(part))
            .then_some(parts)
    }
}
