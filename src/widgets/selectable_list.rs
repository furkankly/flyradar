use dashmap::DashSet;
use ratatui::widgets::TableState as State;

/// List widget with TUI controlled states.
#[derive(Debug)]
pub struct SelectableList {
    /// List items.
    pub items: Vec<Vec<String>>,
    /// Current filtered items.
    // INFO: Not owning this propagates lifetimes up to the background task which I dont feel like
    // dealing with rn. We could just store indices and do index-based filtering to not .clone but
    // this is just okkkkk.
    pub filtered_items: Vec<Vec<String>>,
    /// Current search filter that can be modified by TUI.
    pub search_filter: String,
    /// State (selection) that can be modified by TUI.
    /// This always opts on filtered_items, because that's whats shown on UI.
    pub state: State,
    /// State that's used when the multi-select mode is on to act on multiple items for certain
    /// use-cases.
    pub multi_select_state: DashSet<String>,
}

impl Default for SelectableList {
    fn default() -> Self {
        Self::with_items(Vec::new())
    }
}

impl SelectableList {
    /// Constructs a new instance of `SelectableList`.
    pub fn new(
        items: Vec<Vec<String>>,
        filtered_items: Vec<Vec<String>>,
        mut state: State,
        search_filter: String,
        multi_select_state: DashSet<String>,
    ) -> SelectableList {
        state.select(Some(0));
        Self {
            items,
            filtered_items,
            search_filter,
            state,
            multi_select_state,
        }
    }

    /// Construct a new `SelectableList` with given items.
    pub fn with_items(items: Vec<Vec<String>>) -> SelectableList {
        let filtered_items = items.clone();
        Self::new(
            items,
            filtered_items,
            State::default(),
            String::default(),
            DashSet::new(),
        )
    }

    pub fn reset(&mut self) {
        self.items = Vec::new();
        self.filtered_items = Vec::new();
        self.search_filter = String::default();
        self.state = State::default();
        self.state.select(Some(0));
        self.multi_select_state = DashSet::new();
    }

    pub fn apply_search_filter(&mut self, new_search_filter: &str) {
        let new_filtered_items: Vec<Vec<String>> = self
            .items
            .iter()
            .filter(|&row| row.iter().any(|s| s.contains(new_search_filter)))
            .cloned()
            .collect();

        // Select the first element.
        self.state.select(Some(0));
        self.filtered_items = new_filtered_items;
        self.search_filter = new_search_filter.to_string();
    }

    pub fn set_items(&mut self, new_items: Vec<Vec<String>>, prev_selected_id: Option<String>) {
        let new_filtered_items: Vec<Vec<String>> = new_items
            .iter()
            .filter(|&row| row.iter().any(|s| s.contains(&self.search_filter)))
            .cloned()
            .collect();

        // INFO:Adjust the selection based on prev selected id (new resource view)
        let mut new_selected = prev_selected_id
            .and_then(|id| new_filtered_items.iter().position(|item| item[0] == id))
            .or(Some(0));

        // INFO:Adjust the selection in case there were deletions between fetches (same resource view)
        if !self.filtered_items.is_empty() {
            let current_selected_item = self.selected();

            new_selected = current_selected_item.and_then(|current_selected_item| {
                new_filtered_items
                    .iter()
                    .position(|item| item[0] == current_selected_item[0])
                    .or(Some(0))
            });
        }

        self.state.select(new_selected);
        self.items = new_items;
        self.filtered_items = new_filtered_items;
        //TODO:Adjust multi_select_state in case there were deletions between fetches
    }

    /// Returns the selected item.
    pub fn selected(&self) -> Option<&Vec<String>> {
        self.filtered_items.get(self.state.selected()?)
    }

    /// Selects the first item.
    pub fn first(&mut self) {
        self.state.select(Some(0));
    }

    /// Selects the last item.
    pub fn last(&mut self) {
        self.state
            .select(Some(self.filtered_items.len().saturating_sub(1)));
    }

    /// Selects the next item.
    pub fn next(&mut self, amount: usize) {
        let i = match self.state.selected() {
            Some(i) => {
                if i.saturating_add(amount) >= self.filtered_items.len() {
                    0
                } else {
                    i.saturating_add(amount)
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Selects the previous item.
    pub fn previous(&mut self, amount: usize) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.filtered_items.len().saturating_sub(1)
                } else {
                    i.saturating_sub(amount)
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Toggles for multi-select.
    pub fn toggle_multi_selection(&mut self) {
        if let Some(key) = self.selected().map(|row| row[0].clone()) {
            if self.multi_select_state.contains(&key) {
                self.multi_select_state.remove(&key);
            } else {
                self.multi_select_state.insert(key);
            }
        }
    }
}
