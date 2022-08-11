# Game Boy Rust

This project is Rust implementation of GameBoy emulator for learning Rust language.

## 遊び方

このリポジトリをcloneし、以下のコマンドを実行してください。

`cargo run <Full PATH for ROM binary>`

なおキーコンフィグですが、作者の使っているキーボード配列が特殊なため以下のようになっています。  
もしも不満がある場合はmain.rsの107行目から154行目を適宜書き換えてください。

| Game Boy | KeyBoard | 
| -------- | -------- | 
| UP       | E        | 
| DOWN     | D        | 
| LEFT     | S        | 
| RIGHT    | F        | 
| A        | K        | 
| B        | J        | 
| SELECT   | Space    | 
| START    | Enter    | 
