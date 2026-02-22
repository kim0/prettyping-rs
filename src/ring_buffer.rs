#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingBuffer<T> {
    data: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T> RingBuffer<T> {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: std::iter::repeat_with(|| None).take(capacity).collect(),
            head: 0,
            len: 0,
        }
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, value: T) -> Option<T> {
        let capacity = self.capacity();
        if capacity == 0 {
            return Some(value);
        }

        if self.len < capacity {
            let index = (self.head + self.len) % capacity;
            self.data[index] = Some(value);
            self.len += 1;
            return None;
        }

        let index = self.head;
        self.head = (self.head + 1) % capacity;
        self.data[index].replace(value)
    }

    #[must_use]
    pub fn iter(&self) -> RingBufferIter<'_, T> {
        RingBufferIter {
            buffer: self,
            offset: 0,
        }
    }
}

pub struct RingBufferIter<'a, T> {
    buffer: &'a RingBuffer<T>,
    offset: usize,
}

impl<'a, T> Iterator for RingBufferIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.buffer.len {
            return None;
        }

        let index = (self.buffer.head + self.offset) % self.buffer.capacity();
        self.offset += 1;
        self.buffer.data[index].as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::RingBuffer;

    #[test]
    fn overwrites_oldest_value_when_full() {
        let mut ring = RingBuffer::with_capacity(3);

        assert_eq!(ring.push(1), None);
        assert_eq!(ring.push(2), None);
        assert_eq!(ring.push(3), None);
        assert_eq!(ring.push(4), Some(1));

        let values: Vec<i32> = ring.iter().copied().collect();
        assert_eq!(values, vec![2, 3, 4]);
    }

    #[test]
    fn empty_capacity_discards_values() {
        let mut ring = RingBuffer::with_capacity(0);

        assert_eq!(ring.push(42), Some(42));
        assert!(ring.is_empty());
        assert_eq!(ring.len(), 0);
        assert_eq!(ring.capacity(), 0);
    }
}
