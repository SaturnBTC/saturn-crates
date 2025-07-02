#[derive(Debug)]
pub struct FixedSet<const SIZE: usize> {
    included: [bool; SIZE],
    count: usize,
}

impl<const SIZE: usize> Default for FixedSet<SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize> FixedSet<SIZE> {
    pub fn new() -> Self {
        Self {
            included: [false; SIZE],
            count: 0,
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn contains(&self, index: usize) -> bool {
        index < SIZE && self.included[index]
    }

    pub fn insert(&mut self, index: usize) -> bool {
        if index < SIZE && !self.included[index] {
            self.included[index] = true;
            self.count += 1;
            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index < SIZE && self.included[index] {
            self.included[index] = false;
            self.count -= 1;
            true
        } else {
            false
        }
    }

    pub fn extend_from_slice(&mut self, indices: &[usize]) {
        for &index in indices {
            self.insert(index);
        }
    }

    /// Collect all included indices into a sorted array, returning actual count
    pub fn collect_sorted(&self, buffer: &mut [usize; SIZE]) -> usize {
        let mut count = 0;
        for (index, &included) in self.included.iter().enumerate() {
            if included && count < SIZE {
                buffer[count] = index;
                count += 1;
            }
        }
        count
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.included
            .iter()
            .enumerate()
            .filter_map(|(index, &included)| if included { Some(index) } else { None })
    }

    pub fn clear(&mut self) {
        self.included.fill(false);
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: usize = 8;

    #[test]
    fn test_new_and_default_are_empty() {
        let set = FixedSet::<SIZE>::new();
        assert_eq!(set.count(), 0);
        assert!(set.is_empty());

        let default_set: FixedSet<SIZE> = Default::default();
        assert_eq!(default_set.count(), 0);
        assert!(default_set.is_empty());
    }

    #[test]
    fn test_insert_and_contains() {
        let mut set = FixedSet::<SIZE>::new();
        assert!(!set.contains(3));
        assert!(set.insert(3));
        assert!(set.contains(3));
        assert_eq!(set.count(), 1);
        assert!(!set.is_empty());

        // Duplicate insert
        assert!(!set.insert(3));
        assert_eq!(set.count(), 1);
    }

    #[test]
    fn test_insert_out_of_bounds() {
        let mut set = FixedSet::<SIZE>::new();
        assert!(!set.insert(SIZE));
        assert_eq!(set.count(), 0);
    }

    #[test]
    fn test_remove() {
        let mut set = FixedSet::<SIZE>::new();
        set.insert(2);
        set.insert(5);
        assert!(set.remove(2));
        assert!(!set.contains(2));
        assert_eq!(set.count(), 1);

        // Removing non-included index
        assert!(!set.remove(2));
        assert_eq!(set.count(), 1);
    }

    #[test]
    fn test_remove_out_of_bounds() {
        let mut set = FixedSet::<SIZE>::new();
        assert!(!set.remove(SIZE));
    }

    #[test]
    fn test_extend_from_slice() {
        let mut set = FixedSet::<SIZE>::new();
        set.extend_from_slice(&[1, 3, 5, 3]); // 3 is a duplicate
        assert!(set.contains(1));
        assert!(set.contains(3));
        assert!(set.contains(5));
        assert_eq!(set.count(), 3);
    }

    #[test]
    fn test_collect_sorted() {
        let mut set = FixedSet::<SIZE>::new();
        set.insert(4);
        set.insert(1);
        set.insert(7);

        let mut buffer = [0; SIZE];
        let count = set.collect_sorted(&mut buffer);
        assert_eq!(count, 3);
        assert_eq!(&buffer[..count], &[1, 4, 7]);
    }

    #[test]
    fn test_iter() {
        let mut set = FixedSet::<SIZE>::new();
        let indices = [2, 4, 6];
        set.extend_from_slice(&indices);

        let collected: Vec<_> = set.iter().collect();
        assert_eq!(collected, indices);
    }

    #[test]
    fn test_clear() {
        let mut set = FixedSet::<SIZE>::new();
        set.insert(0);
        set.insert(1);
        assert_eq!(set.count(), 2);
        set.clear();
        assert_eq!(set.count(), 0);
        assert!(set.is_empty());
        for i in 0..SIZE {
            assert!(!set.contains(i));
        }
    }
}
