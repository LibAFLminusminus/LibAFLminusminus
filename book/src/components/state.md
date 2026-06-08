# State
State is, the data storage that we store all the data related to the currently running fuzzer.
This include the corpus (for storing non-crashing interesting testcases), the objective corpus (for storing crashing testcases), the metadata for saving persistent data.
Remember, all the data stored inside the `State` is persistent across fuzzer restarts. 
Data living outside `State` is lost forever.
If you want to make your data persistent across restarts, you have to store the data in `State`, and most likely, you want to save them inside the metadata map.

# Corpus

The Corpus is where `Testcase`s are stored. We define a `Testcase` structure to hold the input along with other minor metadatas such as ids and file format specifiers.

A Corpus can store testcases in different ways, for example on disk (`OnDiskCorpus`), or in memory (`InMemoryCorpus`), or implement a cache to speedup on disk storage (`CachedOnDiskCorpus).

Usually, a testcase is added to the Corpus when it is considered as interesting, but a Corpus is used also to store testcases that fulfill an objective (like crashing the program under test for instance).
For this reason, we have objective corpus for saving crashes.

Related to the Corpus is the way in which the next testcase (the fuzzer would ask for) is retrieved from the Corpus. The taxonomy for this handling in LibAFL is `Scheduler`, the entity representing the policy to fetch testcases from the Corpus.
For now, we have `QueueScheduler` that takes and pushes testcases in a FIFO manner, but you can implement your own policy of picking testcases

# Component relationship

`State` contains both corpus and objective corpus. Both are `Corpus` object.
Each `Corpus` object contains a `Scheduler` object.