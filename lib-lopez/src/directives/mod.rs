mod aggregator;
mod parse;

pub use parse::{Aggregator, Boundary, Extractor, Item, Literal, RuleSet};

use regex::RegexSet;
use scraper::Html;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fmt;
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

    /// Finds invalide set-variable names within this module.
    fn find_invalid_set_variables(&self, invalid: &mut HashSet<String>) {
        for item in &self.items {
            if let Item::SetVariable(set_variable) = item {
                if Variable::try_parse(&set_variable.name).is_none() {
                    invalid.insert(set_variable.name.clone());
                }
            }
        }
    }

    /// Finds duplicate set-variable names within this module.
    fn find_duplicate_set_variables(
        &self,
        set_variables: &mut HashSet<String>,
        duplicates: &mut HashSet<String>,
    ) {
        for item in &self.items {
            if let Item::SetVariable(set_variable) = item {
                if !set_variables.insert(set_variable.name.clone()) {
                    duplicates.insert(set_variable.name.clone());
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

    /// Finds invalid set variables.
    fn find_invalid_set_variables(&self) -> HashSet<String> {
        let mut invalid = HashSet::new();

        for (_name, module) in &self.modules {
            module.find_invalid_set_variables(&mut invalid);
        }

        invalid
    }

    /// Finds duplicate set variables.
    fn find_duplicate_set_variables(&self) -> HashSet<String> {
        let mut set_variables = HashSet::new();
        let mut duplicates = HashSet::new();

        for (_name, module) in &self.modules {
            module.find_duplicate_set_variables(&mut set_variables, &mut duplicates);
        }

        duplicates
    }

    /// Validates set-variables types. After this, you can always unwrap errors
    /// on `SetVariable`.
    fn find_bad_set_variable_values(&self) -> Vec<crate::Error> {
        let variables = self.set_variables();
        let tests = vec![
            variables.get_as_str(Variable::UserAgent).err(),
            variables.get_as_usize(Variable::Quota).err(),
            variables.get_as_usize(Variable::MaxDepth).err(),
            variables.get_as_positive_f64(Variable::MaxHitsPerSec).err(),
            variables
                .get_as_positive_f64(Variable::RequestTimeout)
                .err(),
            variables.get_as_usize(Variable::MaxBodySize).err(),
        ];

        tests
            .into_iter()
            .filter_map(|maybe_err| maybe_err)
            .collect()
    }

    /// Validates if all directives "are sound". Returns an error message if
    /// any error is found.
    fn validate(&self) -> Result<(), String> {
        let mut issues = vec![];
        let duplicates = self.find_duplicate_rules();
        if !duplicates.is_empty() {
            issues.push(format!(
                "There are duplicated rules in directives: \n\t- {}",
                duplicates.into_iter().collect::<Vec<_>>().join("\n\t- ")
            ));
        }

        let invalid_seeds = self.find_invalid_seeds();
        if !invalid_seeds.is_empty() {
            issues.push(format!(
                "There are seeds on the frontier or outside your boundaries: \n\t- {}",
                invalid_seeds
                    .into_iter()
                    .map(|url| url.as_str().to_owned())
                    .collect::<Vec<_>>()
                    .join("\n\nt- ")
            ));
        }

        let invalid = self.find_invalid_set_variables();
        if !invalid.is_empty() {
            issues.push(format!(
                "There are invalid set-variable definitions \
                (these name are not known): \n\t- {}",
                invalid.into_iter().collect::<Vec<_>>().join("\n\t- "),
            ));
        }

        let duplicates = self.find_duplicate_set_variables();
        if !duplicates.is_empty() {
            issues.push(format!(
                "There are duplicate set-variable definitions \
                (these definitions are global): \n\t- {}",
                duplicates.into_iter().collect::<Vec<_>>().join("\n\t- "),
            ));
        }

        let bad_values = self.find_bad_set_variable_values();
        if !bad_values.is_empty() {
            issues.push(format!(
                "There are bad values for set-variables: \n\t- {}",
                bad_values
                    .into_iter()
                    .map(|err| err.to_string())
                    .collect::<Vec<_>>()
                    .join("\n\nt"),
            ))
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

    pub fn set_variables(&self) -> SetVariables {
        let set_variables = self
            .modules
            .iter()
            .flat_map(|(_module_name, module)| {
                module.items.iter().filter_map(move |item| {
                    if let Item::SetVariable(set_variable) = item {
                        Some((
                            Variable::try_parse(&set_variable.name)?,
                            set_variable.value.clone(),
                        ))
                    } else {
                        None
                    }
                })
            })
            .collect::<BTreeMap<Variable, Literal>>();

        SetVariables { set_variables }
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
                    (
                        // Top-level directives don't get the dot.
                        if module_name != "" {
                            module_name.to_owned() + SEPARATOR + name
                        } else {
                            name.to_owned()
                        },
                        state.finalize(),
                    )
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Variable {
    UserAgent,
    Quota,
    MaxDepth,
    MaxHitsPerSec,
    RequestTimeout,
    MaxBodySize,
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Variable::UserAgent => "user_agent",
                Variable::Quota => "quota",
                Variable::MaxDepth => "max_depth",
                Variable::MaxHitsPerSec => "max_hits_per_sec",
                Variable::RequestTimeout => "request_timeout",
                Variable::MaxBodySize => "max_body_size",
            }
        )
    }
}

impl Variable {
    fn try_parse(input: &str) -> Option<Variable> {
        Some(match input {
            "user_agent" => Variable::UserAgent,
            "quota" => Variable::Quota,
            "max_depth" => Variable::MaxDepth,
            "max_hits_per_sec" => Variable::MaxHitsPerSec,
            "request_timeout" => Variable::RequestTimeout,
            "max_body_size" => Variable::MaxBodySize,
            _ => return None,
        })
    }

    fn bad_value(&self, literal: &Literal) -> crate::Error {
        crate::Error::BadSetVariableValue(self.clone(), literal.clone())
    }

    fn retrieve_as_str<'a>(&self, literal: Option<&'a Literal>) -> Result<&'a str, crate::Error> {
        match (self, literal) {
            (Variable::UserAgent, None) => Ok(crate::cli::default_user_agent()),
            (Variable::UserAgent, Some(Literal::String(user_agent))) => Ok(&*user_agent),
            (Variable::UserAgent, Some(literal)) => Err(self.bad_value(literal)),
            _ => panic!("cannot cast as string: {:?}", self),
        }
    }

    // TODO: when "or patterns" stabilize, refactor this code.

    fn retrieve_as_positive_f64(&self, literal: Option<&Literal>) -> Result<f64, crate::Error> {
        match (self, literal) {
            (Variable::MaxHitsPerSec, None) => Ok(2.5),
            (Variable::RequestTimeout, None) => Ok(60.0),
            (Variable::MaxHitsPerSec, Some(Literal::Number(number))) if *number > 0. => Ok(*number),
            (Variable::RequestTimeout, Some(Literal::Number(number))) if *number > 0. => {
                Ok(*number)
            }
            (Variable::MaxHitsPerSec, Some(literal)) => Err(self.bad_value(literal)),
            (Variable::RequestTimeout, Some(literal)) => Err(self.bad_value(literal)),
            _ => panic!("cannot cast as float: {:?}", self),
        }
    }

    fn retrieve_as_usize(&self, literal: Option<&Literal>) -> Result<usize, crate::Error> {
        match (self, literal) {
            (Variable::Quota, None) => Ok(1000),
            (Variable::MaxDepth, None) => Ok(7),
            (Variable::MaxBodySize, None) => Ok(10_000_000),
            (Variable::Quota, Some(Literal::Number(number)))
                if *number > 0. && number.fract() == 0. =>
            {
                Ok(*number as usize)
            }
            (Variable::MaxDepth, Some(Literal::Number(number)))
                if *number > 0. && number.fract() == 0. =>
            {
                Ok(*number as usize)
            }
            (Variable::MaxBodySize, Some(Literal::Number(number)))
                if *number > 0. && number.fract() == 0. =>
            {
                Ok(*number as usize)
            }
            (Variable::Quota, Some(literal)) => Err(self.bad_value(literal)),
            (Variable::MaxDepth, Some(literal)) => Err(self.bad_value(literal)),
            (Variable::MaxBodySize, Some(literal)) => Err(self.bad_value(literal)),
            _ => panic!("cannot cast as usize: {:?}", self),
        }
    }
}

#[derive(Debug)]
pub struct SetVariables {
    set_variables: BTreeMap<Variable, Literal>,
}

impl SetVariables {
    pub fn get_as_str(&self, name: Variable) -> Result<&str, crate::Error> {
        name.retrieve_as_str(self.set_variables.get(&name))
    }

    pub fn get_as_positive_f64(&self, name: Variable) -> Result<f64, crate::Error> {
        name.retrieve_as_positive_f64(self.set_variables.get(&name))
    }

    pub fn get_as_usize(&self, name: Variable) -> Result<usize, crate::Error> {
        name.retrieve_as_usize(self.set_variables.get(&name))
    }
}
