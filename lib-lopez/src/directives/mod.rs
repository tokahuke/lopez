mod aggregator;
mod parse;

pub use parse::{Aggregator, Boundary, Extractor, Item, RuleSet};

use regex::RegexSet;
use scraper::Html;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use url::Url;

use self::aggregator::AggregatorState;

const SEPARATOR: &str = ".";
const EXTENSION: &str = "lcd";
const MODULE_FILE: &str = "module";

/// Reads from a list of possible paths and returns at the first not-not-found
/// (there might be other errors). Returns not found if none matches.
fn read_from_many<P: AsRef<Path>>(paths: &[P]) -> Result<(&P, String), io::Error> {
    for path in paths {
        match fs::read_to_string(path.as_ref()) {
            Ok(content) => return Ok((path, content)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }

    Err(io::Error::from(io::ErrorKind::NotFound))
}

/// Loads items for a given module from a list of possible paths.
fn load_items_from<'a, P: AsRef<Path>>(
    module_name: &str,
    paths: &'a [P],
) -> Result<(&'a P, Vec<Item>), String> {
    let (path, module_str) = read_from_many(paths)
        .map_err(|err| format!("could not open module `{}`: {}", module_name, err))?;

    let module = parse::entrypoint(&module_str)
        .map_err(|err| format!("failed to parse `{}`: {}", module_name, err))?
        .1
        .map_err(|err| format!("failed to interpret `{}`: {}", module_name, err))?;

    Ok((path, module))
}

/// Strips `supers` and `roots` from a module path. Returns errors if put into
/// an impossible position.
fn canonical_path(path: &str) -> Result<String, String> {
    let mut parts = vec![];
    for part in path.split(SEPARATOR) {
        match part {
            "super" if path.is_empty() => return Err(format!("got empty path from `{}`", path)),
            "super" => {
                parts.pop();
            }
            "root" => parts.clear(),
            "" => {}
            part => parts.push(part),
        }
    }

    Ok(parts.join(SEPARATOR))
}

/// Directives for Lopez.
#[derive(Debug)]
pub struct Directives {
    modules: BTreeMap<String, Module>,
}

/// A module of directives.
#[derive(Debug)]
struct Module {
    items: Vec<Item>,
}

impl Module {
    /// Finds duplicates names for scraping rules within this modules.
    fn find_duplicate_rules(
        &self,
        prefix: String,
        rule_names: &mut HashSet<String>,
        duplicates: &mut HashSet<String>,
    ) {
        // Find all rule names:
        // let mut rule_names = HashSet::new();
        // let mut duplicates = HashSet::new();
        for item in &self.items {
            if let Item::RuleSet(rule_set) = item {
                for rule_name in rule_set.aggregators.keys() {
                    let prefixed = prefix.clone() + SEPARATOR + rule_name;
                    if !rule_names.insert(prefixed.clone()) {
                        duplicates.insert(prefixed);
                    }
                }
            }
        }
    }

    /// Loads a module and its dependencies into a set of modules.
    fn load<P: AsRef<Path>, Q: AsRef<Path>>(
        roots: &[P],
        module_name: String,
        modules: &mut BTreeMap<String, Module>,
        paths: &[Q],
    ) -> Result<(), String> {
        if modules.contains_key(&module_name) {
            return Ok(());
        }

        let (_path, items) = load_items_from(&module_name, paths)?;

        for item in &items {
            if let Item::Module(module) = item {
                let sub_module_name =
                    canonical_path(&(module_name.to_owned() + SEPARATOR + &module.path))?;

                let paths = roots
                    .iter()
                    .flat_map(|root| {
                        let file_path = root
                            .as_ref()
                            .to_owned()
                            .join(sub_module_name.split(SEPARATOR).collect::<PathBuf>());
                        let folder_path = file_path.join(MODULE_FILE);

                        vec![
                            file_path.with_extension(EXTENSION),
                            folder_path.with_extension(EXTENSION),
                        ]
                    })
                    .collect::<Vec<_>>();

                Self::load(roots, sub_module_name, modules, &paths)?;
            }
        }

        modules.insert(module_name, Module { items });

        Ok(())
    }
}

impl Directives {
    /// Finds duplicates names for scraping rules within all modules.
    fn find_duplicate_rules(&self) -> HashSet<String> {
        let mut rule_names = HashSet::new();
        let mut duplicates = HashSet::new();

        for (name, module) in &self.modules {
            module.find_duplicate_rules(name.to_owned(), &mut rule_names, &mut duplicates);
        }

        duplicates
    }

    /// Finds seeds that are outside bounds.
    /// TODO: implement "disallowed by regex so-and-so".
    fn find_invalid_seeds(&self) -> Vec<Url> {
        // This impl. is dumb, but works:
        let seeds = self.seeds();
        let boundaries = self.boundaries();

        seeds
            .into_iter()
            .map(|url| boundaries.filter_query_params(url))
            .filter(|url| !boundaries.is_allowed(url) || boundaries.is_frontier(url))
            .collect::<Vec<_>>()
    }

    /// Validates if all directives "are sound". Returns an error message if
    /// any error is found.
    fn validate(&self) -> Result<(), String> {
        let mut issues = vec![];
        let duplicates = self.find_duplicate_rules();
        if !duplicates.is_empty() {
            issues.push(format!(
                "There are duplicated rules in directives: \n\t{}",
                duplicates.into_iter().collect::<Vec<_>>().join("\n\t- ")
            ));
        }

        let invalid_seeds = self.find_invalid_seeds();
        if !invalid_seeds.is_empty() {
            issues.push(format!(
                "There are seeds on the frontier or outside your boundaries: \n\t{}",
                invalid_seeds
                    .into_iter()
                    .map(|url| url.as_str().to_owned())
                    .collect::<Vec<_>>()
                    .join("\n\nt- ")
            ));
        }

        if !issues.is_empty() {
            return Err(issues.join("\n"));
        }

        Ok(())
    }

    /// Loads directives from a given file while also loading all dependencies.
    pub fn load<P: AsRef<Path>, Q: AsRef<Path>>(path: P, imports: Q) -> Result<Self, String> {
        let parent = path.as_ref().parent().expect("cannot be root");
        let mut modules = BTreeMap::new();

        Module::load(
            &[parent, imports.as_ref()],
            "".to_owned(),
            &mut modules,
            &[path.as_ref()],
        )?;

        let directives = Directives { modules };

        directives.validate()?;

        Ok(directives)
    }

    /// Returns all seeds loaded for this directives.
    pub fn seeds(&self) -> Vec<Url> {
        self.modules
            .values()
            .flat_map(|module| &module.items)
            .filter_map(|item| {
                if let Item::Seed(seed) = item {
                    Some(seed.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn boundaries(&self) -> Boundaries {
        let mut allowed = vec![];
        let mut disallowed = vec![];
        let mut frontier = vec![];
        let mut use_params = vec![];
        let mut ignore_params = vec![];
        let mut use_all_params = false;

        self.modules
            .values()
            .flat_map(|module| &module.items)
            .filter_map(|item| {
                if let Item::Boundary(boundary) = item {
                    Some(boundary)
                } else {
                    None
                }
            })
            .for_each(|boundary| match boundary {
                Boundary::Allowed(allowed_rx) => allowed.push(allowed_rx.as_str()),
                Boundary::Disallowed(disallowed_rx) => disallowed.push(disallowed_rx.as_str()),
                Boundary::Frontier(frontier_rx) => frontier.push(frontier_rx.as_str()),
                Boundary::UseParam(param) => use_params.push(param.to_owned()),
                Boundary::IgnoreParam(param) => ignore_params.push(param.to_owned()),
                Boundary::UseAllParams => use_all_params = true,
            });

        Boundaries {
            allowed: RegexSet::new(allowed).expect("regex's from set have already bee validated"),
            disallowed: RegexSet::new(disallowed)
                .expect("regex's from set have already bee validated"),
            frontier: RegexSet::new(frontier).expect("regex's from set have already bee validated"),
            use_params,
            ignore_params,
            use_all_params,
        }
    }

    // Gets the absolute names of all rules.
    pub fn rule_names(&self) -> Vec<String> {
        self.modules
            .iter()
            .flat_map(|(module_name, module)| {
                module.items.iter().filter_map(move |item| {
                    if let Item::RuleSet(rule_set) = item {
                        Some((module_name, rule_set))
                    } else {
                        None
                    }
                })
            })
            .flat_map(|(module_name, rule_set)| {
                rule_set
                    .aggregators
                    .keys()
                    .map(move |name| module_name.to_owned() + SEPARATOR + name)
            })
            .collect()
    }

    pub fn analyzer(&self) -> Analyzer {
        let rule_sets = self
            .modules
            .iter()
            .flat_map(|(module_name, module)| {
                module.items.iter().filter_map(move |item| {
                    if let Item::RuleSet(rule_set) = item {
                        Some((module_name.to_owned(), rule_set.clone()))
                    } else {
                        None
                    }
                })
            })
            .collect::<Vec<(String, RuleSet)>>();

        Analyzer { rule_sets }
    }
}

#[derive(Debug)]
pub struct Boundaries {
    allowed: RegexSet,
    disallowed: RegexSet,
    frontier: RegexSet,
    /// TODO: use aho-corasick?
    use_params: Vec<String>,
    ignore_params: Vec<String>,
    use_all_params: bool,
}

impl Boundaries {
    pub fn is_allowed<S: AsRef<str>>(&self, url: S) -> bool {
        self.allowed.is_match(url.as_ref()) && !self.disallowed.is_match(url.as_ref())
    }

    pub fn is_frontier<S: AsRef<str>>(&self, url: S) -> bool {
        self.frontier.is_match(url.as_ref())
    }

    pub fn filter_query_params(&self, mut url: Url) -> Url {
        let filtered_pairs = url
            .query_pairs()
            .filter(|(key, _)| {
                (self.use_all_params || self.use_params.iter().any(|use_params| use_params == key))
                    && !self
                        .ignore_params
                        .iter()
                        .any(|ignore_param| ignore_param == key)
            })
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();

        url.query_pairs_mut().clear().extend_pairs(filtered_pairs);

        url
    }
}

#[derive(Debug)]
pub struct Analyzer {
    rule_sets: Vec<(String, RuleSet)>,
}

impl Analyzer {
    pub fn analyze(&self, url: &Url, html: &Html) -> Vec<(String, Value)> {
        self.rule_sets
            .iter()
            .filter(|(_, rule_set)| {
                if let Some(regex) = &rule_set.in_page {
                    !regex.is_match(url.as_str())
                } else {
                    true
                }
            })
            .flat_map(|(module_name, rule_set)| {
                let mut states = rule_set
                    .aggregators
                    .iter()
                    .map(|(name, agg)| (name, AggregatorState::new(agg)))
                    .collect::<Vec<_>>();

                for element_ref in html.select(&rule_set.selector) {
                    for (_, state) in &mut states {
                        state.aggregate(element_ref);
                    }
                }

                states.into_iter().map(move |(name, state)| {
                    (module_name.to_owned() + SEPARATOR + name, state.finalize())
                })
            })
            .collect()
    }
}
