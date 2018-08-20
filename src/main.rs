extern crate colored;
extern crate dotenv;
extern crate egg_mode;
extern crate failure;
extern crate tokio_core;

// TODO: Good error messages when env isn't correct
// TODO: Warning about not using this for anything
// TODO: MIT license?
// TODO: Write Readme - Why do this? Is this serious? Okay, but is it?

// twitter connection inspired by hello: https://github.com/hello-rust/hello

use colored::*;
use dotenv::dotenv;
use failure::Error;
use std::env;
use std::fs;
use std::io;
use std::process::Command;
use tokio_core::reactor::Core;

static TEST_FILE: &'static str = "test.rs";
static TEST_EXE: &'static str = "test.exe";
static TEST_PDB: &'static str = "test.pdb";

fn main() -> Result<(), Error> {
    dotenv().ok();

    let consumer_key = env::var("TWITTER_CONSUMER_KEY")?.to_string();
    let consumer_secret = env::var("TWITTER_CONSUMER_SECRET")?.to_string();
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
        // Not registerd yet. Requires OAuth dance
        _ => Credentials::load(consumer_key, consumer_secret)?,
    };

    let client = Client::new(credentials);

    let count = 5;
    let feed = client.get_tweets(count)?;
    let mut pass_count = 0;
    let mut fail_count = 0;
    println!("Running {} tests", count);
    for tweet in feed {
        let program = tweet.text.clone();
        print!("test {} ({})... ", tweet.id, tweet.created_at);
        fs::write(TEST_FILE, program).expect("Unable to write program to file.");
        //println!("<@{}> {}", tweet.user.as_ref().unwrap().screen_name, tweet.text);

        let output = Command::new("rustc")
            .args(&["-A", "dead_code", "-A", "non_camel_case_types", TEST_FILE, "-o", TEST_EXE])
            .output()
            .expect("Failed to execute rustc");

        if output.status.success() {
            pass_count += 1;
            println!("{}", "ok".green());
        } else {
            fail_count += 1;
            println!("{}", "FAILED".red());
            let encoded = String::from_utf8_lossy(output.stderr.as_slice());
            println!("{}", encoded);
        }
    }

    fs::remove_file(TEST_FILE).expect("Could not delete test file.");
    fs::remove_file(TEST_EXE).expect("Could not delete test executable.");
    fs::remove_file(TEST_PDB).expect("Could not delete test pdb.");

    let result = if fail_count > 0 { "FAILED".red() } else { "SUCCESS".green() };
    println!("\ntest result: {}. {} passed; {} failed", result, pass_count, fail_count);

    Ok(())
}

#[test]
pub fn test_something() {

}

#[test]
pub fn test_something_else() {
    assert!(false);
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

    pub fn get_tweets (&self, count : i32) -> Result<Vec<egg_mode::tweet::Tweet>, Error> {
        let mut core = Core::new()?;
        let handle = core.handle();

        let timeline = egg_mode::tweet::user_timeline("@everyrust", false, false, &self.credentials.token, &handle).with_page_size(count);

        let (_, feed) = core.run(timeline.start())?;
        Ok(feed.response)
    }
}