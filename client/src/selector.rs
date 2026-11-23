use shared::ColumnIndex;

/// tracks which column the player currently has highlighted, if any
#[derive(Clone, Copy, Default)]
pub struct ColumnSelector(Option<ColumnIndex>);

impl ColumnSelector {
    pub fn selected(self) -> Option<ColumnIndex> {
        self.0
    }

    pub fn move_left(&mut self) {
        self.0 = Some(
            self.0
                .map_or_else(ColumnIndex::left_most, ColumnIndex::decrement),
        );
    }

    pub fn move_right(&mut self) {
        self.0 = Some(
            self.0
                .map_or_else(ColumnIndex::right_most, ColumnIndex::increment),
        );
    }

    pub fn select(&mut self, column: ColumnIndex) {
        self.0 = Some(column);
    }
}
