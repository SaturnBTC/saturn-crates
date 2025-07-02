use crate::generic::push_pop::PushPopCollection;

#[derive(Debug)]
pub struct FixedList<T, const SIZE: usize> {
    items: [T; SIZE],
    len: usize,
}

impl<T: Default + Copy, const SIZE: usize> Default for FixedList<T, SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default + Copy, const SIZE: usize> FixedList<T, SIZE> {
    pub fn new() -> Self {
        Self {
            items: [T::default(); SIZE],
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        self.items[..self.len].iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> + '_ {
        self.items[..self.len].iter_mut()
    }

    pub fn push(&mut self, item: T) {
        if self.len < SIZE {
            self.items[self.len] = item;
            self.len += 1;
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len > 0 {
            self.len -= 1;
            Some(self.items[self.len])
        } else {
            None
        }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.items[..self.len]
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.items[..self.len]
    }

    pub fn copy_from_slice(&mut self, slice: &[T])
    where
        T: Clone,
    {
        self.len = slice.len();
        self.items[..self.len].clone_from_slice(slice);
    }
}

impl<T: Default + Copy, const SIZE: usize> PushPopCollection<T> for FixedList<T, SIZE> {
    fn push(&mut self, item: T) {
        self.push(item);
    }

    fn pop(&mut self) -> Option<T> {
        self.pop()
    }

    fn as_slice(&self) -> &[T] {
        self.as_slice()
    }

    fn len(&self) -> usize {
        self.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_empty() {
        let list = FixedList::<u32, 4>::default();
        assert_eq!(list.len(), 0);
        assert!(list.is_empty());
    }

    #[test]
    fn test_push_and_len() {
        let mut list = FixedList::<u32, 3>::new();
        list.push(10);
        list.push(20);
        assert_eq!(list.len(), 2);
        assert!(!list.is_empty());
        assert_eq!(list.as_slice(), &[10, 20]);
    }

    #[test]
    fn test_push_past_capacity() {
        let mut list = FixedList::<u32, 2>::new();
        list.push(1);
        list.push(2);
        list.push(3); // should be ignored
        assert_eq!(list.len(), 2);
        assert_eq!(list.as_slice(), &[1, 2]);
    }

    #[test]
    fn test_pop() {
        let mut list = FixedList::<u32, 2>::new();
        assert_eq!(list.pop(), None);

        list.push(100);
        list.push(200);
        assert_eq!(list.pop(), Some(200));
        assert_eq!(list.pop(), Some(100));
        assert_eq!(list.pop(), None);
        assert!(list.is_empty());
    }

    #[test]
    fn test_iter() {
        let mut list = FixedList::<u32, 3>::new();
        list.push(1);
        list.push(2);
        let collected: Vec<_> = list.iter().copied().collect();
        assert_eq!(collected, vec![1, 2]);
    }

    #[test]
    fn test_iter_mut() {
        let mut list = FixedList::<u32, 3>::new();
        list.push(1);
        list.push(2);
        for x in list.iter_mut() {
            *x *= 10;
        }
        assert_eq!(list.as_slice(), &[10, 20]);
    }

    #[test]
    fn test_as_slice_and_mut_slice() {
        let mut list = FixedList::<u32, 4>::new();
        list.push(42);
        list.push(99);
        let slice = list.as_slice();
        assert_eq!(slice, &[42, 99]);

        let mut_slice = list.as_mut_slice();
        mut_slice[0] = 123;
        assert_eq!(list.as_slice(), &[123, 99]);
    }

    #[test]
    fn test_copy_from_slice() {
        let mut list = FixedList::<u32, 5>::new();
        let data = [9, 8, 7];
        list.copy_from_slice(&data);
        assert_eq!(list.len(), 3);
        assert_eq!(list.as_slice(), &data);
    }

    #[test]
    fn test_push_pop_collection_trait() {
        let mut list = FixedList::<u8, 2>::new();
        PushPopCollection::push(&mut list, 1);
        PushPopCollection::push(&mut list, 2);
        PushPopCollection::push(&mut list, 3); // should be ignored
        assert_eq!(PushPopCollection::len(&list), 2);
        assert_eq!(PushPopCollection::as_slice(&list), &[1, 2]);
        assert_eq!(PushPopCollection::pop(&mut list), Some(2));
        assert_eq!(PushPopCollection::pop(&mut list), Some(1));
        assert_eq!(PushPopCollection::pop(&mut list), None);
    }
}
