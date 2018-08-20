# Test Every Rust

A command-line tool that tests every possible Rust program...eventually.

### Why?

At RustConf 2018, I was chatting with somebody about [Crater](https://github.com/rust-lang-nursery/crater)
and what an amazing tool it is for ensuring compatibility with language changes.
I jokingly lamented that it couldn't test every possible Rust program.

Then, on my drive home, I remembered the [Every Rust](https://twitter.com/everyrust) twitter
where [@carols10cents](https://github.com/carols10cents/) posts a fuzzer-generated Rust program every day. The tagline is:

> Will generate all possible Rust programs eventually, unless the heat death of the universe happens first.

Aha, I thought to myself! This will allow me to test every possible Rust program.

### Are you serious?

I was until you asked that so condescendingly and made me feel bad!
Seriously, though, this is mostly a toy I made to get more Rust experience and play with
some new APIs. It's not *actually* intended to test every Rust program ever made.

...Or is it?

### Should I run this thing?

Almost certainly not. And definitely not without a sandboxed environment. It downlods
and builds arbitrary code from the internet. Let's not be ridiculous.

And while Test Every Rust doesn't actually RUN that code, just builds it, uh...still,
let's not be silly. To rip off the Crater warning:

:warning: **DO NOT RUN 'TEST EVERY RUST' IN AN UNSANDBOXED ENVIRONMENT** :warning:  
Test Every Rust builds potentially malicious code that will destroy what you love.

### Usage

Create a `.env` file with your credentials. See `.env_dist` for an example.

After that, run `cargo run` to build every rust.

You can also `cargo run 1031277598507192320` to build a specific tweet by ID.

We use whatever the `rustc` in your path is to build, so you can use it to test any rustc! Amazing!

### Concessions

Currently, every tweet builds successfully. To get that to work I had to make some concessions:

* Dead code is allowed. Many tweets have dead code.
* Non-camel-case types are allowed. Many tweets have non-camel-case code.
* Addition and multiplication overflow is allowed. SO MANY TWEETS have overflow.
* All code is built as a lib, as many tweets do not have main() functions.

### How to get your Twitter credentials

Make an app for yourself at apps.twitter.com and add the credentials to the
`.env` file. On first execution, you will get an access token for your app via OAuth.

Follow the instructions on the screen to save that for all subsequent requests.

### Shout Outs

The twitter functionality herein was made possible by egg-mode, and the specific
implmentation...erm...'borrows' heavily from [Hello](https://github.com/hello-rust/hello).