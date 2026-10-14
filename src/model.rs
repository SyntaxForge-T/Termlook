//! Widget-free session model. The on-disk format remains compatible with 0.1.x.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

pub type Id = u64;
pub fn id() -> Id {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
pub fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/".into())
}
fn directory(value: &Value) -> String {
    value
        .as_str()
        .filter(|p| Path::new(p).is_dir())
        .map(str::to_owned)
        .unwrap_or_else(home)
}
fn name(value: &Value, default: &str) -> String {
    value
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(default)
        .chars()
        .filter(|c| *c != '\0')
        .take(80)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    Horizontal,
    Vertical,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Tree {
    Leaf(Id),
    Split {
        axis: Axis,
        ratio: f64,
        first: Box<Tree>,
        second: Box<Tree>,
    },
}
impl Tree {
    pub fn branch(axis: Axis, a: Self, b: Self) -> Self {
        Self::Split {
            axis,
            ratio: 0.5,
            first: Box::new(a),
            second: Box::new(b),
        }
    }
    pub fn default_for(ids: &[Id]) -> Self {
        match ids.len() {
            1 => Self::Leaf(ids[0]),
            2 => Self::branch(Axis::Horizontal, Self::Leaf(ids[0]), Self::Leaf(ids[1])),
            _ => Self::branch(
                Axis::Vertical,
                Self::default_for(&ids[..2]),
                Self::default_for(&ids[2..]),
            ),
        }
    }
    pub fn leaves(&self) -> Vec<Id> {
        match self {
            Self::Leaf(id) => vec![*id],
            Self::Split { first, second, .. } => [first.leaves(), second.leaves()].concat(),
        }
    }
    pub fn replace(&mut self, id: Id, replacement: &Self) {
        match self {
            Self::Leaf(i) if *i == id => *self = replacement.clone(),
            Self::Split { first, second, .. } => {
                first.replace(id, replacement);
                second.replace(id, replacement);
            }
            _ => {}
        }
    }
    pub fn remove(self, id: Id) -> Option<Self> {
        match self {
            Self::Leaf(i) => (i != id).then_some(Self::Leaf(i)),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => match (first.remove(id), second.remove(id)) {
                (Some(a), Some(b)) => Some(Self::Split {
                    axis,
                    ratio,
                    first: Box::new(a),
                    second: Box::new(b),
                }),
                (a, b) => a.or(b),
            },
        }
    }
    pub fn set_ratio(&mut self, path: &[bool], value: f64) {
        if let Self::Split {
            ratio,
            first,
            second,
            ..
        } = self
        {
            if let Some((right, rest)) = path.split_first() {
                if *right {
                    second.set_ratio(rest, value);
                } else {
                    first.set_ratio(rest, value);
                }
            } else {
                *ratio = value.clamp(0.05, 0.95);
            }
        }
    }
    fn encode(&self, ids: &[Id]) -> Value {
        match self {
            Self::Leaf(id) => json!(ids.iter().position(|i| i == id).unwrap_or(0)),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                json!({"axis": axis, "ratio": ratio, "first": first.encode(ids), "second": second.encode(ids)})
            }
        }
    }
    fn decode(v: &Value, ids: &[Id], depth: usize) -> Option<Self> {
        if depth > 3 {
            return None;
        }
        if let Some(i) = v.as_u64() {
            return ids.get(usize::try_from(i).ok()?).map(|id| Self::Leaf(*id));
        }
        let axis = serde_json::from_value(v.get("axis")?.clone()).ok()?;
        let ratio = v.get("ratio").map_or(Some(0.5), Value::as_f64)?;
        if !(0.05..=0.95).contains(&ratio) {
            return None;
        }
        Some(Self::Split {
            axis,
            ratio,
            first: Box::new(Self::decode(v.get("first")?, ids, depth + 1)?),
            second: Box::new(Self::decode(v.get("second")?, ids, depth + 1)?),
        })
    }
}
#[derive(Clone, Debug)]
pub struct Pane {
    pub id: Id,
    pub cwd: String,
}
impl Pane {
    pub fn new(cwd: String) -> Self {
        Self { id: id(), cwd }
    }
}
#[derive(Clone, Debug)]
pub struct Tab {
    pub id: Id,
    pub name: String,
    pub panes: Vec<Pane>,
    pub tree: Tree,
    pub active: Id,
    pub zoomed: bool,
}
impl Tab {
    pub fn new(name: &str, cwd: String) -> Self {
        let pane = Pane::new(cwd);
        let active = pane.id;
        Self {
            id: id(),
            name: name.into(),
            panes: vec![pane],
            tree: Tree::Leaf(active),
            active,
            zoomed: false,
        }
    }
    pub fn active_pane(&self) -> &Pane {
        self.panes
            .iter()
            .find(|p| p.id == self.active)
            .unwrap_or(&self.panes[0])
    }
    pub fn split(&mut self, axis: Axis) {
        if self.panes.len() >= 4 {
            return;
        }
        let pane = Pane::new(self.active_pane().cwd.clone());
        self.tree.replace(
            self.active,
            &Tree::branch(axis, Tree::Leaf(self.active), Tree::Leaf(pane.id)),
        );
        self.active = pane.id;
        self.panes.push(pane);
        self.zoomed = false;
    }
    pub fn close_pane(&mut self, pane: Id) {
        if self.panes.len() <= 1 || !self.panes.iter().any(|p| p.id == pane) {
            return;
        }
        self.tree = self
            .tree
            .clone()
            .remove(pane)
            .expect("another pane remains");
        self.panes.retain(|p| p.id != pane);
        if self.active == pane {
            self.active = self.panes[0].id;
        }
        if self.panes.len() == 1 {
            self.zoomed = false;
        }
    }
    pub fn swap(&mut self, a: Id, b: Id) {
        if a == b || !self.panes.iter().any(|p| p.id == a) || !self.panes.iter().any(|p| p.id == b)
        {
            return;
        }
        self.tree.replace(a, &Tree::Leaf(0));
        self.tree.replace(b, &Tree::Leaf(a));
        self.tree.replace(0, &Tree::Leaf(b));
    }
    pub fn grid(&mut self) {
        while self.panes.len() < 4 {
            self.split(Axis::Horizontal);
        }
        self.tree = Tree::default_for(&self.panes.iter().map(|p| p.id).collect::<Vec<_>>());
        self.zoomed = false;
    }
}
#[derive(Clone, Debug)]
pub struct Workspace {
    pub profile: String,
    pub id: Id,
    pub name: String,
    pub tabs: Vec<Tab>,
    pub selected: usize,
}
impl Workspace {
    pub fn new(name: &str) -> Self {
        Self {
            id: id(),
            name: name.into(),
            profile: String::new(),
            tabs: vec![Tab::new("main", home())],
            selected: 0,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Layout {
    pub workspaces: Vec<Workspace>,
    pub selected: usize,
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            workspaces: vec![Workspace::new("work")],
            selected: 0,
        }
    }
}
impl Layout {
    pub fn tab(&self) -> &Tab {
        let w = &self.workspaces[self.selected];
        &w.tabs[w.selected]
    }
    pub fn tab_mut(&mut self) -> &mut Tab {
        let w = &mut self.workspaces[self.selected];
        &mut w.tabs[w.selected]
    }
    pub fn find_tab_mut(&mut self, id: Id) -> Option<&mut Tab> {
        self.workspaces
            .iter_mut()
            .flat_map(|w| &mut w.tabs)
            .find(|t| t.id == id)
    }
    pub fn close_tab(&mut self, id: Id) {
        for w in &mut self.workspaces {
            if let Some(index) = w.tabs.iter().position(|t| t.id == id) {
                let tab = w.tabs.remove(index);
                if w.tabs.is_empty() {
                    w.tabs.push(Tab::new("main", tab.active_pane().cwd.clone()));
                }
                if index < w.selected {
                    w.selected -= 1;
                }
                w.selected = w.selected.min(w.tabs.len() - 1);
                return;
            }
        }
    }
    pub fn remove_workspace(&mut self, id: Id) {
        if let Some(index) = self.workspaces.iter().position(|w| w.id == id) {
            self.workspaces.remove(index);
            if self.workspaces.is_empty() {
                self.workspaces.push(Workspace::new("work"));
            }
            if index < self.selected {
                self.selected -= 1;
            }
            self.selected = self.selected.min(self.workspaces.len() - 1);
        }
    }
    pub fn live_ids(&self) -> HashSet<Id> {
        self.workspaces
            .iter()
            .flat_map(|w| &w.tabs)
            .flat_map(|t| &t.panes)
            .map(|p| p.id)
            .collect()
    }
    pub fn from_data(v: &Value) -> Self {
        let mut result = Self {
            workspaces: vec![],
            selected: 0,
        };
        for w in v.as_array().into_iter().flatten().take(50) {
            let mut workspace = Workspace::new(&name(&w["name"], "work"));
            workspace.profile = name(&w["profile"], "");
            workspace.tabs.clear();
            for t in w["tabs"].as_array().into_iter().flatten().take(50) {
                let mut tab = Tab::new(&name(&t["name"], "main"), directory(&t["cwd"]));
                for p in t["panes"].as_array().into_iter().flatten().take(3) {
                    tab.panes.push(Pane::new(directory(&p["cwd"])));
                }
                let ids: Vec<_> = tab.panes.iter().map(|p| p.id).collect();
                tab.tree = Tree::decode(&t["split"], &ids, 0)
                    .filter(|tree| {
                        let mut leaves = tree.leaves();
                        leaves.sort_unstable();
                        let mut sorted = ids.clone();
                        sorted.sort_unstable();
                        leaves == sorted
                    })
                    .unwrap_or_else(|| Tree::default_for(&ids));
                workspace.tabs.push(tab);
            }
            if workspace.tabs.is_empty() {
                workspace.tabs.push(Tab::new("main", home()));
            }
            result.workspaces.push(workspace);
        }
        if result.workspaces.is_empty() {
            Self::default()
        } else {
            result
        }
    }
    pub fn to_data(&self) -> Value {
        Value::Array(self.workspaces.iter().map(|w| {
            let mut workspace = json!({"name": w.name, "tabs": w.tabs.iter().map(|t| {
                let mut entry = json!({"name": t.name, "cwd": t.panes[0].cwd});
                if t.panes.len() > 1 {
                    entry["panes"] = json!(t.panes[1..].iter().map(|p| json!({"cwd": p.cwd})).collect::<Vec<_>>());
                    entry["split"] = t.tree.encode(&t.panes.iter().map(|p| p.id).collect::<Vec<_>>());
                }
                entry
            }).collect::<Vec<_>>()});
            if !w.profile.is_empty() {workspace["profile"] = json!(w.profile);}
            workspace
        }).collect())
    }
}
