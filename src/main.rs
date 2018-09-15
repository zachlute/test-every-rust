extern crate clap;
extern crate colored;
extern crate ctrlc;
extern crate dotenv;
extern crate egg_mode;
extern crate failure;
extern crate tokio_core;

use clap::{App, Arg};
use colored::*;
use dotenv::dotenv;
use failure::Error;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio_core::reactor::Core;

static OUTPUT_DIR: &str = "./output";
static STATUS_CONTROL_C_EXIT: i32 = -1073741510;

fn main() {
    dotenv().ok();

    let matches = App::new("Test Every Rust")
        .version("0.1")
        .author("Zach Lute <zach.lute@gmail.com>")
        .about("Ensures programs from the Every Rust twitter account build.")
        .arg(
            Arg::with_name("TWEET_ID")
                .help("Builds a specific tweet.")
                .required(false)
                .index(1),
        ).get_matches();

    let consumer_key = env::var("TWITTER_CONSUMER_KEY")
        .expect("TWITTER_CONSUMER_KEY not defined in environment or .env file.")
        .to_string();
    let consumer_secret = env::var("TWITTER_CONSUMER_SECRET")
        .expect("TWITTER_CONSUMER_SECRET not defined in environment or .env file.")
        .to_string();
    let credentials = match (
        env::var("TWITTER_ACCESS_KEY"),
        env::var("TWITTER_ACCESS_SECRET"),
    ) {
        // Already registered
        (Ok(access_token_key), Ok(access_token_secret)) => Credentials::new(
            consumer_key,
            consumer_secret,
            access_token_key,
            access_token_secret,
        ),
        // Not registered yet. Requires OAuth dance
        _ => Credentials::load(consumer_key, consumer_secret).expect("Could not load credentials."),
    };

    let client = Client::new(credentials);

    let results = Arc::new(Mutex::new(ResultData::new()));

    if Path::new(OUTPUT_DIR).exists() {
        fs::remove_dir_all(OUTPUT_DIR).expect("Could not remove output directory.");
    }
    fs::create_dir(OUTPUT_DIR).expect("Could not create output directory.");

    let cancelled = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));

    if let Some(tweet_id) = matches.value_of("TWEET_ID") {
        if let Ok(tweet_id) = tweet_id.parse::<u64>() {
            println!("Running 1 test");

            results.lock().unwrap().total_count = 1;

            let tweet = client
                .get_tweet(tweet_id)
                .expect("Could not retrieve tweet.");

            match &tweet.user {
                Some(user) => {
                    match user.screen_name.as_ref() {
                        "everyrust" => {
                            // Everything is fine!
                        }
                        _ => {
                            panic!("Tweet was not by @everyrust");
                        }
                    }
                }
                None => {
                    panic!("No user specified.");
                }
            }

            match build_tweet(&tweet, None) {
                Ok(_) => {
                    results.lock().unwrap().pass_count += 1;
                }
                Err(e) => {
                    let mut r = results.lock().unwrap();
                    r.fail_count += 1;
                    r.failures.push((tweet.id, e));
                }
            }
        } else {
            panic!("Invalid Tweet ID: {}", tweet_id);
        }
    } else {
        // Install a Ctrl+C handler so we can gracefully exist.
        let ctrlc_cancelled = Arc::clone(&cancelled);
        let ctrlc_finished = Arc::clone(&finished);
        ctrlc::set_handler(move || {
            // Mark us cancelled.
            ctrlc_cancelled.store(true, Ordering::SeqCst);

            // Wait for the other thread to stop.
            while !ctrlc_finished.load(Ordering::SeqCst) {}
        }).expect("Error setting Ctrl-C handler");

        let ignore_list = get_ignore_list();

        let mut oldest_id = None;

        let count = client.get_tweet_count().expect("Could not retrieve tweet count.");
        results.lock().unwrap().total_count = count;

        // The count isnt exact because it includes retweets if any exist.
        println!("Running ~{} tests", count); 

        loop {
            let feed = client
                .get_latest_tweets(oldest_id)
                .expect("Could not retrieve tweets.");

            if feed.is_empty() {
                break;
            }

            for tweet in feed {
                // We always want tweets older than the oldest,
                // so we subtract one, because otherwise we'll get
                // the oldest back in the next query.
                oldest_id = Some(tweet.id - 1);

                if ignore_list.contains(&tweet.id) {
                    results.lock().unwrap().ignore_count += 1;
                    continue;
                }

                match build_tweet(&tweet, Some(&cancelled)) {
                    Ok(_) => {
                        results.lock().unwrap().pass_count += 1;
                    }
                    Err(e) => {
                        let mut r = results.lock().unwrap();
                        r.fail_count += 1;
                        r.failures.push((tweet.id, e));
                    }
                }

                if cancelled.load(Ordering::SeqCst) {
                    // We've been terminated. Stop doing things.
                    break;
                }
            }

            if cancelled.load(Ordering::SeqCst) {
                // We've been terminated. Stop doing things.
                break;
            }
        }
    }

    print_results(&results.lock().unwrap(), cancelled.load(Ordering::SeqCst));

    cleanup();

    // Mark us finished.
    finished.store(true, Ordering::SeqCst);
}

struct ResultData {
    failures: Vec<(u64, String)>,
    pass_count: i32,
    fail_count: i32,
    ignore_count: i32,
    total_count: i32,
}

impl ResultData {
    pub fn new() -> ResultData {
        ResultData {
            failures: Vec::new(),
            pass_count: 0,
            fail_count: 0,
            ignore_count: 0,
            total_count: 0,
        }
    }
}

fn cleanup () {
    fs::remove_dir_all(OUTPUT_DIR).expect("Could not remove output directory.");
}

fn print_results (results: &ResultData, cancelled: bool) {
    if !results.failures.is_empty() {
        println!("\nfailures:\n");

        for f in &results.failures {
            println!("---- {} stderr ----", f.0);
            println!("{}", f.1);
        }

        println!("failures:");

        for f in &results.failures {
            println!("    {}", f.0);
        }
    }

    let aggregate_result = if results.fail_count > 0 {
        "FAILED".red()
    } else if cancelled {
        "ABORTED".yellow()
    } else {
        "ok".green()
    };

    print!(
        "\ntest result: {}. {} passed; {} failed; {} ignored",
        aggregate_result, results.pass_count, results.fail_count, results.ignore_count
    );

    if cancelled {
        println!("; {} aborted", results.total_count - results.pass_count - results.fail_count - results.ignore_count);
    } else {
        println!();
    }
}

fn get_ignore_list() -> HashSet<u64> {
    let mut result = HashSet::new();
    result.insert(574310847759040512); // Prose tweet, not code.
    result.insert(574285011484020736); // Prose tweet, not code.
    result
}

fn build_tweet(tweet: &egg_mode::tweet::Tweet, cancelled: Option<& Arc<AtomicBool>>) -> Result<(), String> {
    let program = tweet.text.replace("&amp;", "&");
    print!("test {} ({}) ... ", tweet.id, tweet.created_at);

    let test_file = format!("{}/{}.rs", OUTPUT_DIR, tweet.id);
    let test_output = format!("{}/{}.output", OUTPUT_DIR, tweet.id);
    let test_pdb = format!("{}/{}.pdb", OUTPUT_DIR, tweet.id);

    fs::write(&test_file, program).expect("Unable to write program to file.");

    let output = Command::new("rustc")
        .args(&[
            "-A",
            "dead_code",
            "-A",
            "non_camel_case_types",
            "-A",
            "const_err",
            "--crate-type=lib",
            &test_file,
            "-o",
            &test_output,
        ]).output()
        .expect("Failed to execute rustc");

    if Path::new(&test_file).exists() {
        fs::remove_file(test_file).expect("Could not delete test file.");
    }

    if Path::new(&test_output).exists() {
        fs::remove_file(test_output).expect("Could not delete test executable.");
    }

    if Path::new(&test_pdb).exists() {
        fs::remove_file(test_pdb).expect("Could not delete test pdb.");
    }

    if let Some(cancelled) = cancelled {
        if cancelled.load(Ordering::SeqCst) {
            println!("ABORTED");
            return Ok(());
        }
    }

    if let Some(code) = output.status.code() {
        if code == STATUS_CONTROL_C_EXIT {
            println!("{}", "ABORTED".yellow());
            return Err("ABORTED!".to_string())
        }
    }

    if output.status.success() {
        println!("{}", "ok".green());
        Ok(())
    } else {
        println!("{}", "FAILED".red());
        Err(String::from_utf8_lossy(output.stderr.as_slice()).to_string())
    }
}

pub struct Credentials {
    pub token: egg_mode::Token,
}

impl Credentials {
    pub fn new(
        consumer_key: String,
        consumer_secret: String,
        access_token_key: String,
        access_token_secret: String,
    ) -> Credentials {
        let con_token = egg_mode::KeyPair::new(consumer_key, consumer_secret);
        let access_token = egg_mode::KeyPair::new(access_token_key, access_token_secret);
        let token = egg_mode::Token::Access {
            consumer: con_token,
            access: access_token,
        };
        Credentials { token }
    }

    /// If we don't have an access token already (e.g. if the application is not
    /// registered, grab one via OAuth.
    pub fn load(consumer_key: String, consumer_secret: String) -> Result<Credentials, Error> {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let con_token = egg_mode::KeyPair::new(consumer_key, consumer_secret);

        let request_token = core.run(egg_mode::request_token(&con_token, "oob", &handle))?;

        println!("Go to the following URL, sign in, and give me the PIN that comes back:");
        println!("{}", egg_mode::authorize_url(&request_token));
        println!("Type in PIN here:");

        let mut pin = String::new();
        io::stdin().read_line(&mut pin)?;

        let (token, _user_id, _screen_name) = core.run(egg_mode::access_token(
            con_token,
            &request_token,
            pin,
            &handle,
        ))?;

        match token {
            egg_mode::Token::Access {
                access: ref access_token,
                ..
            } => {
                println!("Please add the following to your `.env` file:");
                println!("TWITTER_ACCESS_KEY={}", &access_token.key);
                println!("TWITTER_ACCESS_SECRET={}", &access_token.secret);
            }
            _ => unreachable!(),
        }

        Ok(Credentials { token })
    }
}

pub struct Client {
    credentials: Credentials,
}

impl Client {
    pub fn new(credentials: Credentials) -> Client {
        Client { credentials }
    }

    pub fn get_tweet(&self, tweet_id: u64) -> Result<egg_mode::tweet::Tweet, Error> {
        let mut core = Core::new()?;
        let handle = core.handle();

        let tweet = egg_mode::tweet::show(tweet_id, &self.credentials.token, &handle);
        let result = core.run(tweet)?;

        Ok(result.response)
    }

    pub fn get_tweet_count(&self) -> Result<i32, Error> {
        let mut core = Core::new()?;
        let handle = core.handle();

        let user = egg_mode::user::show("@everyrust", &self.credentials.token, &handle);
        let result = core.run(user)?;

        Ok(result.response.statuses_count)
    }

    pub fn get_latest_tweets(
        &self,
        older_than_id: Option<u64>,
    ) -> Result<Vec<egg_mode::tweet::Tweet>, Error> {
        let mut core = Core::new()?;
        let handle = core.handle();

        if let Some(id) = older_than_id {
            let timeline = egg_mode::tweet::user_timeline(
                "@everyrust",
                false,
                false,
                &self.credentials.token,
                &handle,
            );
            let (_, feed) = core.run(timeline.newer(Some(id)))?;
            Ok(feed.response)
        } else {
            let timeline = egg_mode::tweet::user_timeline(
                "@everyrust",
                false,
                false,
                &self.credentials.token,
                &handle,
            );

            let (_, feed) = core.run(timeline.start())?;
            Ok(feed.response)
        }
    }
}
