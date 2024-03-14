//! A trie map stores a value with each word or key.
use super::{Trie, Value};
use crate::map::inc_search::IncSearch;
use crate::map::postfix_iter::PostfixIter;
use crate::map::prefix_iter::PrefixIter;
use crate::map::search_iter::SearchIter;
use frayed::Defray;
use louds_rs::{self, ChildNodeIter, LoudsNodeNum};

impl<Label: Ord, Value> Trie<Label, Value> {
    /// Return `Some(&value)` if query is an exact match.
    pub fn exact_match<L>(&self, query: impl AsRef<[L]>) -> Option<&Value>
    where
        Label: PartialOrd<L>,
    {
        self.exact_match_node(query)
            .and_then(move |x| self.value(x))
    }

    /// Return `Node` if query is an exact match.
    #[inline]
    fn exact_match_node<L>(&self, query: impl AsRef<[L]>) -> Option<LoudsNodeNum>
    where
        Label: PartialOrd<L>,
    {
        let mut cur_node_num = LoudsNodeNum(1);

        for (i, chr) in query.as_ref().iter().enumerate() {
            let children_node_nums: Vec<LoudsNodeNum> =
                self.children_node_nums(cur_node_num).collect();
            let res = self.bin_search_by_children_labels(chr, &children_node_nums[..]);

            match res {
                Ok(j) => {
                    let child_node_num = children_node_nums[j];
                    if i == query.as_ref().len() - 1 && self.is_terminal(child_node_num) {
                        return Some(child_node_num);
                    }
                    cur_node_num = child_node_num;
                }
                Err(_) => return None,
            }
        }
        None
    }

    /// Return `Some(&mut value)` if query is an exact match.
    pub fn exact_match_mut<L>(&mut self, query: impl AsRef<[L]>) -> Option<&mut Value>
    where
        Label: PartialOrd<L>,
    {
        self.exact_match_node(query)
            .and_then(move |x| self.value_mut(x))
    }

    /// Create an incremental search. Useful for interactive applications.
    pub fn inc_search(&self) -> IncSearch<'_, Label, Value> {
        IncSearch::new(self)
    }

    /// Return true if `query` is a prefix.
    ///
    /// Note: A prefix may be an exact match or not, and an exact match may be a
    /// prefix or not.
    pub fn is_prefix<L>(&self, query: impl AsRef<[L]>) -> bool
    where
        Label: PartialOrd<L>,
    {
        let mut cur_node_num = LoudsNodeNum(1);

        for chr in query.as_ref().iter() {
            let children_node_nums: Vec<_> = self.children_node_nums(cur_node_num).collect();
            let res = self.bin_search_by_children_labels(chr, &children_node_nums[..]);
            match res {
                Ok(j) => cur_node_num = children_node_nums[j],
                Err(_) => return false,
            }
        }
        // Are there more nodes after our query?
        self.has_children_node_nums(cur_node_num)
    }

    /// Return all entries and their values that match `query`, cloned.
    ///
    /// # Panics
    /// If `query` is empty.
    pub fn predictive_search<L>(&self, query: impl AsRef<[L]>) -> Vec<(Vec<Label>, Value)>
    where
        Label: PartialOrd<L> + Clone,
        Value: Clone,
    {
        let chunk = self.predictive_search_ref(query);
        chunk
            .map(|mut v| {
                (
                    v.by_ref().cloned().collect(),
                    v.value().cloned().unwrap(),
                )
            })
            .into_iter()
            .collect()
    }

    /// Return all entries and their values that match `query`.
    ///
    /// # Panics
    /// If `query` is empty.
    pub fn predictive_search_ref<L>(
        &self,
        query: impl AsRef<[L]>,
    ) -> Defray<SearchIter<'_, Label, Value>>
    where
        Label: PartialOrd<L>,
    {
        assert!(!query.as_ref().is_empty());
        let mut cur_node_num = LoudsNodeNum(1);
        let mut prefix = Vec::new();

        // Consumes query (prefix)
        for chr in query.as_ref() {
            let children_node_nums: Vec<_> = self.children_node_nums(cur_node_num).collect();
            let res = self.bin_search_by_children_labels(chr, &children_node_nums[..]);
            match res {
                Ok(i) => cur_node_num = children_node_nums[i],
                Err(_) => return Defray::new(SearchIter::empty(self)),
            }
            prefix.push(cur_node_num);
        }
        let _ = prefix.pop();
        Defray::new(SearchIter::new(self, prefix, cur_node_num))
    }

    /// Return the postfixes and values of all entries that match `query`, cloned.
    ///
    /// # Panics
    /// If `query` is empty.
    pub fn postfix_search<L>(&self, query: impl AsRef<[L]>) -> Vec<(Vec<Label>, Value)>
    where
        Label: PartialOrd<L> + Clone,
        Value: Clone,
    {
        let chunk = self.postfix_search_ref(query);
        chunk
            .map(|mut v| {
                (
                    v.by_ref().cloned().collect(),
                    v.value().cloned().unwrap()
                )
            })
            .into_iter()
            .collect()
    }

    /// Return the postfixes and values of all entries that match `query`.
    ///
    /// # Panics
    /// If `query` is empty.
    pub fn postfix_search_ref<L>(
        &self,
        query: impl AsRef<[L]>,
    ) -> Defray<PostfixIter<'_, Label, Value>>
    where
        Label: PartialOrd<L>,
    {
        assert!(!query.as_ref().is_empty());
        let mut cur_node_num = LoudsNodeNum(1);

        // Consumes query (prefix)
        for chr in query.as_ref() {
            let children_node_nums: Vec<_> = self.children_node_nums(cur_node_num).collect();
            let res = self.bin_search_by_children_labels(chr, &children_node_nums[..]);
            match res {
                Ok(i) => cur_node_num = children_node_nums[i],
                Err(_) => {
                    return Defray::new(PostfixIter::empty(self));
                }
            }
        }
        Defray::new(PostfixIter::new(self, cur_node_num))
    }

    /// Return the common prefixes of `query`, cloned.
    pub fn common_prefix_search<L>(&self, query: impl AsRef<[L]>) -> Vec<(Vec<Label>, Value)>
    where
        Label: PartialOrd<L> + Clone,
        L: Clone,
        Value: Clone,
    {
        let chunk = self.common_prefix_search_ref(query.as_ref());
        chunk
            .map(|mut v| {
                (
                    v.by_ref().cloned().collect(),
                    v.value().cloned().unwrap()
                )
            })
            .into_iter()
            .collect()
    }

    /// Return the common prefixes and values of `query`.
    pub fn common_prefix_search_ref<L>(
        &self,
        query: impl AsRef<[L]>,
    ) -> Defray<PrefixIter<'_, L, Label, Value>>
    where
        Label: PartialOrd<L>,
        L: Clone,
    {
        Defray::new(PrefixIter::new(self, query.as_ref().to_vec()))
    }

    // fn wrap_group<I: Iterator>(iter: I) -> MyIter

    pub(crate) fn has_children_node_nums(&self, node_num: LoudsNodeNum) -> bool {
        self.louds
            .parent_to_children_indices(node_num)
            .next()
            .is_some()
    }

    pub(crate) fn children_node_nums(&self, node_num: LoudsNodeNum) -> ChildNodeIter {
        self.louds.parent_to_children_nodes(node_num)
    }

    pub(crate) fn bin_search_by_children_labels<L>(
        &self,
        query: &L,
        children_node_nums: &[LoudsNodeNum],
    ) -> Result<usize, usize>
    where
        Label: PartialOrd<L>,
    {
        children_node_nums.binary_search_by(|child_node_num| {
            self.label(*child_node_num).partial_cmp(query).unwrap()
        })
    }

    pub(crate) fn label(&self, node_num: LoudsNodeNum) -> &Label {
        &self.trie_labels[(node_num.0 - 2) as usize].label
    }

    pub(crate) fn is_terminal(&self, node_num: LoudsNodeNum) -> bool {
        self.trie_labels[(node_num.0 - 2) as usize].value.is_some()
    }

    pub(crate) fn value(&self, node_num: LoudsNodeNum) -> Option<&Value> {
        self.trie_labels[(node_num.0 - 2) as usize].value.as_ref()
    }

    pub(crate) fn value_mut(&mut self, node_num: LoudsNodeNum) -> Option<&mut Value> {
        self.trie_labels[(node_num.0 - 2) as usize].value.as_mut()
    }
}

#[cfg(test)]
mod search_tests {
    use crate::map::{Trie, TrieBuilder};

    fn build_trie() -> Trie<u8, u8> {
        let mut builder = TrieBuilder::new();
        builder.push("a", 0);
        builder.push("app", 1);
        builder.push("apple", 2);
        builder.push("better", 3);
        builder.push("application", 4);
        builder.push("アップル🍎", 5);
        builder.build()
    }

    #[test]
    fn sanity_check() {
        let trie = build_trie();
        assert_eq!(
            trie.predictive_search("apple"),
            vec![("apple".as_bytes().to_vec(), 2)]
        );
    }

    #[test]
    fn value_mut() {
        let mut trie = build_trie();
        assert_eq!(trie.exact_match("apple"), Some(&2));
        let v = trie.exact_match_mut("apple").unwrap();
        *v = 10;
        assert_eq!(trie.exact_match("apple"), Some(&10));
    }

    mod exact_match_tests {
        macro_rules! parameterized_tests {
            ($($name:ident: $value:expr,)*) => {
            $(
                #[test]
                fn $name() {
                    let (query, expected_match) = $value;
                    let trie = super::build_trie();
                    let result = trie.exact_match(query);
                    assert_eq!(result, expected_match);
                }
            )*
            }
        }

        parameterized_tests! {
            t1: ("a", Some(&0)),
            t2: ("app", Some(&1)),
            t3: ("apple", Some(&2)),
            t4: ("application", Some(&4)),
            t5: ("better", Some(&3)),
            t6: ("アップル🍎", Some(&5)),
            t7: ("appl", None),
            t8: ("appler", None),
        }
    }

    mod is_prefix_tests {
        macro_rules! parameterized_tests {
            ($($name:ident: $value:expr,)*) => {
            $(
                #[test]
                fn $name() {
                    let (query, expected_match) = $value;
                    let trie = super::build_trie();
                    let result = trie.is_prefix(query);
                    assert_eq!(result, expected_match);
                }
            )*
            }
        }

        parameterized_tests! {
            t1: ("a", true),
            t2: ("app", true),
            t3: ("apple", false),
            t4: ("application", false),
            t5: ("better", false),
            t6: ("アップル🍎", false),
            t7: ("appl", true),
            t8: ("appler", false),
            t9: ("アップル", true),
        }
    }

    mod predictive_search_tests {
        macro_rules! parameterized_tests {
            ($($name:ident: $value:expr,)*) => {
            $(
                #[test]
                fn $name() {
                    let (query, expected_results) = $value;
                    let trie = super::build_trie();
                    let results: Vec<(String, u8)> = trie.predictive_search(query).into_iter().map(|g| (String::from_utf8(g.0).unwrap(), g.1)).collect();
                                                  // .collect::<Vec<_>>();
                    let expected_results: Vec<(String, u8)> = expected_results.iter().map(|s| (s.0.to_string(), s.1)).collect();
                    assert_eq!(results, expected_results);
                }
            )*
            }
        }

        parameterized_tests! {
            t1: ("a", vec![("a", 0), ("app", 1), ("apple", 2), ("application", 4)]),
            t2: ("app", vec![("app", 1), ("apple", 2), ("application", 4)]),
            t3: ("appl", vec![("apple", 2), ("application", 4)]),
            t4: ("apple", vec![("apple", 2)]),
            t5: ("b", vec![("better", 3)]),
            t6: ("c", Vec::<(&str, u8)>::new()),
            t7: ("アップ", vec![("アップル🍎", 5)]),
        }
    }

    mod common_prefix_search_tests {
        macro_rules! parameterized_tests {
            ($($name:ident: $value:expr,)*) => {
            $(
                #[test]
                fn $name() {
                    let (query, expected_results) = $value;
                    let trie = super::build_trie();
                    let results = trie.common_prefix_search(query);
                    let expected_results: Vec<(Vec<u8>, u8)> = expected_results.iter().map(|s| (s.0.as_bytes().to_vec(), s.1)).collect();
                    assert_eq!(results, expected_results);
                }
            )*
            }
        }

        parameterized_tests! {
            t1: ("a", vec![("a", 0)]),
            t2: ("ap", vec![("a", 0)]),
            t3: ("appl", vec![("a", 0), ("app", 1)]),
            t4: ("appler", vec![("a", 0), ("app", 1), ("apple", 2)]),
            t5: ("bette", Vec::<(&str, u8)>::new()),
            t6: ("betterment", vec![("better", 3)]),
            t7: ("c", Vec::<(&str, u8)>::new()),
            t8: ("アップル🍎🍏", vec![("アップル🍎", 5)]),
        }
    }
}
