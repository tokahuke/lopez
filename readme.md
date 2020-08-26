
# Welcome to "the Lopez"

Crawling the Web for fun and profit.

## Installing the damn thing

Go to the releases section for this repository on github and download the `entalator`, our proto-distribution management system. Execute the file with `sudo` (if you trust this random stranger in the internet) and voilà! you have `lopez` installed globally, together with `lopez-std` to get you started. Ruing `entalator` with the `-u` flag uninstalls `lopez` without ever leaving a trace. The `entalator` should work on any Unix-based system; there is an open issue for porting it to Windows. 

If you wish to run `lopez` on any other architecture, you may compile if from the source code in the repository. 

## Running the damn thing

If you installed from the `entalator`, you are ready to go. However, if you insist on using the source code, you will need Rust Ecosystem, which is easy to install. Just go to https://rust-lang.org/tools/install and follow the instructions. After that, go the `lopez` folder and run:
``` bash
cargo run --release -- --help
```
To get some help on Command Line Interface usage.

Or else, just download a release for your platform

## Lopez Crawl Directives

You will need a Crawl Directives file to run the crawl. This file describes what you want to scrape from web pages as well as _were_ and _how_ you want to crawl the Web. Here is a nice example (yes, syntax highlighting is supported for VSCode!):

![Sample code example for Lopez Crawl Directives](/img/sample-code.png)

Maybe one day, I can write an in-depth tutorial. For now, it is not that complex. Most is covered in the figure above.

## Backends

Lopez supports the idea of backends, which is where the data comes from and goes to. The implementation is completely generic, so you may write your own if you so wish. For now, lopez ships with a nice PostgreSQL backend for your convenience. Support for other popular databases (and unpopular ones as well) is greatly appreciated.

For more information on backends, see the documentation for the `lib_lopez::backend` module.

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
