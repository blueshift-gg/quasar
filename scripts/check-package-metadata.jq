.packages[]
| select(.publish != [])
| . as $package
| if (($package.description // "") | length) == 0 then "\($package.name): missing description" else empty end,
  if $package.homepage != $homepage then "\($package.name): unexpected homepage" else empty end,
  if $package.repository != $repository then "\($package.name): unexpected repository" else empty end,
  if $package.documentation != ($docs + "/" + $package.name + "/" + $package.version) then "\($package.name): documentation must target its exact version" else empty end,
  if (($package.readme // "") | length) == 0 then "\($package.name): missing README metadata" else empty end,
  if (($package.keywords | length) < 1 or ($package.keywords | length) > 5) then "\($package.name): expected 1-5 keywords" else empty end,
  if any($package.keywords[]; test("^[A-Za-z0-9][A-Za-z0-9_+\\-]{0,19}$") | not) then "\($package.name): invalid crates.io keyword" else empty end,
  if (($package.categories | length) < 1 or ($package.categories | length) > 5) then "\($package.name): expected 1-5 categories" else empty end,
  if any($package.categories[]; . as $category | ($allowed | index($category)) == null) then "\($package.name): unknown crates.io category" else empty end
