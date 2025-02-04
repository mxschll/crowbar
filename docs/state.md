
+---------------------------- Application -----------------------------+
|                                                                      |
|  +------------------------ SearchState ---------------------------+  |
|  |                                                                |  |
|  |  +------------------- ItemList Component -------------------+  |  |
|  |  |                                                          |  |  |
|  |  |  [Item 1]     -> {id, name, queries[], action}             | | |
|  |  |  [Item 2]*    -> {id, name, queries[], action}         | | |
|  |  |  [...]        -> {id, name, queries[], action}         | | |
|  |  |  [Item n]     -> {id, name, queries[], action}         | | |
|  |  |                                                         | | |
|  |  +---------------------------------------------------------+ | |
|  |                                                               | |
|  |  +------------------- Query Component ---------------------+   | |
|  |  |                                                       |   | |
|  |  |  [Search Query] -> {filterText}                       |   | |
|  |  |  [Query 1]     -> {parameterName, value} ----+       |   | |
|  |  |  [Query n]     -> {parameterName, value} ----+       |   | |
|  |  |                                              |       |   | |
|  |  +---------------------------------------------|-------+   | |
|  |                                                |           | |
|  |  Selected Item's Parameters <------------------+           | |
|  |  {                                                        | |
|  |    currentSelection: Option<Item>,                        | |
|  |    parameterValues: HashMap<String, String>               | |
|  |  }                                                        | |
|  +-----------------------------------------------------------+ |
|                                                                  |
+------------------------------------------------------------------+

Legend:
* = Selected item
-> = Contains/Holds
--> = Data flow
[] = Visual component
{} = Data structure
