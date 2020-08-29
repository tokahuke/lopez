
# Welcome to "the Lopez"

Crawling the Web for fun and profit.

## A word of caution

There is a very tenuous line between a crawl and a DoS attack. Please, be mindful of the crawling speed you inflict on websites! For your convenience, crawling is limited by default to `2.5` hits per second per [origin](https://html.spec.whatwg.org/#origin), which is a good default. You can override this value using the `set max_hits_per_sec` directive in your configuration, but make sure that you will not overload the server (or that you have the permission to do so). Remember: some people's livelihoods depend on these websites and not every site has good DoS mitigation. 

Also, some people may get angry that _you_ are scraping _their_ website and may start annoying you because of that. If they are crazy enough or money is involved, they may even try to prosecute you. And the judicial system is just crazy nowadays, so who knows?

In either case, [I have nothing to do with that](/license). Use this program at your own risk.

## Installing the damn thing

If you are feeling particularly lazy today, just copy and paste the following in your favorite command line (Unix-like only):
```bash
curl -L "https://github.com/tokahuke/lopez/releases/latest/download/entalator" \
    > /tmp/entalator
chmod +x /tmp/entalator
sudo /tmp/entalator &&
sudo cp /tmp/entalator /usr/share/lopez
```
You will get the latest Lopez experience, which is `lopez` installed for all users in your computer with full access to `lopez-std` out of the box. If you ever wish to get rid of the installation, just use the following one-liner:
```bash
sudo /usr/share/lopez/entalator --uninstall
```
but remember there is no turning back.

This method should work on any Unix-based system; there is an [open issue](https://github.com/tokahuke/lopez/issues/4) for porting it to Windows.  However, with a bit more of setup, you can run `lopez` on most architectures. Compiling from the source code in the repository using Cargo (the Rust package manager) should be quite simple.

## Running the damn thing

If you installed from the `entalator`, you will have the binary `lopez` available globally on your machine. To get started, run
```bash
lopez --help
```
to get a friendly help dialog. This will list your options while running Lopez. To really get started running lopez, see our [Quickstart guide](https://github.com/tokahuke/lopez/wiki/Quickstart).

## Lopez Crawl Directives

You will need a Crawl Directives file to run the crawl. This file describes what you want to scrape from web pages as well as _were_ and _how_ you want to crawl the Web. For more information on the syntax and semantics, see [this link](https://github.com/tokahuke/lopez/wiki/Lopez-Crawl-Directives). Either way, here is a nice example (yes, syntax highlighting is supported for VSCode!):

![Sample code example for Lopez Crawl Directives](/img/sample-code.png)

## Backends

Lopez supports the idea of backends, which is where the data comes from and goes to. The implementation is completely generic, so you may write your own if you so wish. For now, lopez ships with a nice PostgreSQL backend for your convenience. Support for other popular databases (and unpopular ones as well) is greatly appreciated.

For more information on backends, see the documentation for the `lib_lopez::backend` module.

## Minimum Rust Version

By now, Lopez only compiles on Rust Nightly. Unfortunately, we are waiting on the following features to be stabilized:
* `move_ref_pattern`: https://github.com/rust-lang/rust/issues/68354

## Features

Let's brag a little!

* The beast is _fast_, in comparison with other similar programs I have made in the past using the Python ecosystem (BeatufulSoup, asyncio, etc...). It's in Rust; what were you expecting?

* It uses very little memory. If crawling is not done correctly, it can gobble up your memory and still ask for more. Using a database (PostgreSQL), all evil is averted!

* It is polite. Yes, it obeys `robots.txt` and no, you can't turn that off.

## Limitations and future plans

* Lopez is still limited to a single machine. No distributed programming yet. However, what are you crawling that requires so much firepower?

* This crate need more docs and more support for other backends. Sorry, I have a full-time job.

* See the open issues for more scary (and interesting) stuff.

## Licensing

All the work in this repository is licensed under the Apache v2.0 license. See the `license` file for mode detailed information.
