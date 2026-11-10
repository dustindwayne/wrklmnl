//! A grid square owns its tabs independently of the split/docking tree.
#[derive(Clone, Debug)]
pub struct Group {
    pub id: u64,
    pub tabs: Vec<u64>,
    pub active: u64,
}
#[derive(Default)]
pub struct Groups {
    pub items: Vec<Group>,
    next: u64,
}
impl Groups {
    pub fn sync(&mut self, live: &[u64]) {
        for group in &mut self.items {
            group.tabs.retain(|id| live.contains(id));
            if !group.tabs.contains(&group.active) {
                group.active = group.tabs.first().copied().unwrap_or(0);
            }
        }
        self.items.retain(|g| !g.tabs.is_empty());
        for id in live {
            if !self.items.iter().any(|g| g.tabs.contains(id)) {
                self.next += 1;
                self.items.push(Group {
                    id: self.next,
                    tabs: vec![*id],
                    active: *id,
                });
            }
        }
    }
    pub fn add(&mut self, group: u64, tab: u64) -> bool {
        if let Some(g) = self.items.iter_mut().find(|g| g.id == group) {
            g.tabs.push(tab);
            g.active = tab;
            true
        } else {
            false
        }
    }
    pub fn select(&mut self, tab: u64) {
        if let Some(g) = self.items.iter_mut().find(|g| g.tabs.contains(&tab)) {
            g.active = tab;
        }
    }
    pub fn reorder(&mut self, source: u64, target: u64) {
        if let (Some(a), Some(b)) = (
            self.items.iter().position(|g| g.id == source),
            self.items.iter().position(|g| g.id == target),
        ) {
            let group = self.items.remove(a);
            self.items.insert(b, group);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tabs_stay_in_their_square_through_selection_closure_and_reorder() {
        let mut groups = Groups::default();
        groups.sync(&[0, 1]);
        let (a, b) = (groups.items[0].id, groups.items[1].id);
        assert!(groups.add(a, 2));
        groups.sync(&[0, 1, 2]);
        groups.select(0);
        assert_eq!(groups.items[0].tabs, [0, 2]);
        assert_eq!(groups.items[1].tabs, [1]);
        groups.reorder(a, b);
        assert_eq!(groups.items[1].id, a);
        groups.sync(&[1, 2]);
        assert_eq!(groups.items[1].id, a);
        assert_eq!(groups.items[1].active, 2);
        groups.sync(&[1]);
        assert_eq!(groups.items.len(), 1);
        assert_eq!(groups.items[0].id, b);
    }
}
