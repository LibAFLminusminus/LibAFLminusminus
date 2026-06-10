# Tuples

Some of the LibAFL modules allow you to use a group of similar components. 
For example, if you want to use both `TimeObserver` and `MapObserver` to observe both the time and the coverage map, you have to group the two struct together. 
Since they both implement `trait Observer`, you can put them `Vec<Box<dyn Observer>>` but this requires dynamic dispatch. 
In `LibAFL--`, we use tuple lists to avoid it.

# `tuple_list!` macro.
`tuple_list!` macro is a way to group up different object in a nested macro. 
For example, `tuple_list!(A, B, C)` will map to `(A, (B, (C, ())))`

Various modules takes a this object. 
Fuzzer will take a `tuple_list!` of `trait Stages` and `trait FuzzerHook`. `Executor` will take a `tuple_list!` of `trait Observers`.

# MatchNameRef and Handled
`tuple_list!` in `LibAFL--` allows you can group-up objects implementing the same trait, but we also offer an way to find a specific object in your group of objects. `trait MatchNameRef` exposes a `get()` API that you can call on these tuples to find a specific object using a `Handle<T>` object.
Any object that has a name (i.e. implements `trait Named`) implements `Handle<T>` as well, so we can generate a `Handle<T>` object from such object so that we can match it against a tuple later.