# Fonts

Sample fonts used by the Rust tests (`crates/text-shape`), the JS tests (`js/test`)
and the docs site (`site`). They're not part of either published package.

| File                 | Source                                                              | License                                                 |
| -------------------- | ------------------------------------------------------------------- | ------------------------------------------------------- |
| `Roboto-Regular.ttf` | [Roboto](https://github.com/googlefonts/roboto-classic) 3.009       | SIL Open Font License 1.1, see `Roboto-OFL.txt`         |
| `RobotoFlex.ttf`     | [Roboto Flex](https://github.com/googlefonts/roboto-flex), variable | SIL Open Font License 1.1, see `RobotoFlex-OFL.txt`     |
| `NotoSansArabic.ttf` | [Noto Sans Arabic](https://github.com/notofonts/arabic), variable   | SIL Open Font License 1.1, see `NotoSansArabic-OFL.txt` |

Each license file's copyright line matches the notice inside its font. Roboto Flex
and Noto Sans Arabic come from [google/fonts](https://github.com/google/fonts) and
were renamed (the originals list their axes in the file name).

The site ships Roboto Flex, Roboto and Noto Sans Arabic with their license files.
