/// A stable, comparable view of the mock renderer tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotNode {
    Element {
        tag: String,
        namespace: Option<String>,
        attrs: Vec<SnapshotAttr>,
        listeners: Vec<String>,
        children: Vec<SnapshotNode>,
    },
    Text(String),
}

/// A stable attribute snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotAttr {
    pub name: String,
    pub namespace: Option<String>,
    pub value: String,
}
