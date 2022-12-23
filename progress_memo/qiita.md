# Rustで作るゲームボーイエミュレータ

一度はやってみたいと思っていたエミュレータ作成が一段落し、ある程度形になったので、ゲームボーイの仕様解説であったり、感想であったりといった諸々のことを記事として残しておこうと思います。

ちなみに作った時期自体は春から夏にかけてです。コードを書いて概ね満足し、ブログにアウトプットするまでのやる気が出ませんでした。アドカレのネタに出来て良かったです。

純粋な作業期間としてはGitのログを見る限り一ヶ月半から二ヶ月といったところでしょうか。SoundとWASM対応まで含むと三ヶ月かかっているかと思います。ただ途中挫折しかけて二ヶ月から三ヶ月放置したりといったこともあり、first commitからは半年ほど経っていました。

何故今ゲームボーイのエミュレータなどを作ろうと思ったかについてですが、案外多くの人がゲームボーイのエミュレータ実装をしているのを見て、自分でもやってみたいと思ったのがきっかけです。自作OSや自作言語ほど重くなく、それでいて軽すぎもせず、何より作っていて楽しそうだと考えました。作ったところで何かの役に立つわけではありませんが、技術者にとってのある種の「教養」と言えるのではないかと思います。

Rustを採用したのはもう少し即物的な理由で、確かに現状業務では使う機会が訪れないものの、今後を見据えるとある程度習熟しておいた方が良いだろうという考えがあったためです。習熟出来たか？と言われると苦笑せざるを得ませんが、まあ最初はこんなものでしょう。

ということで作ってみた感想ですが、CPUの実装など面倒な作業も含めて楽しかったです。

アルゴリズム的に高度なことや複雑な実装はほぼ要求されませんでした。ネットに転がっている資料を眺めて仕様を把握し、「何をどう実装すればいいのか」を認識する作業が本質だったと思います。実装よりも調査段階で挫折する人の方が多いのではないでしょうか。

作り始めた時はただでさえ実装すべきことが分からないのに、Rustのコンパイラにも怒られまくって脳の負担が大変なことになっていました。競プロのマラソンコンテストとかで事前に慣らしておくべきでしたね。

そして手を焼いたのはやはりデバッグでしょうか。有志によって数多くのテスト用ROMが公開されているのでバグの発見自体は容易です。この辺りゲームボーイエミュレータが初心者向けと言われる所以かなと。

ただ具体的なバグの理由と修正については地道な標準出力デバッグが基本になります。ステップ実行やブレークポイント機能を実装し、コケた位置を把握して、バイナリを逆アセンブルしたROMのソースコードと突き合わせたり、正しい動作をするエミュレータを引っ張ってきて出力を確かめる……といった作業が主になります。まあこの辺の作業はある程度経験のあるプログラマなら泣きながら何度もこなしてきたことかと思います。感覚としては、競プロで何故か一つだけWAが取れずに怒りのprintfデバッグをしている時と同じですね。
エミュレータの実行中、ワンボタンでデバッグモードに入れるようにしたり、RAMやVRAMをダンプ出来るようにしておくと捗ります。
特定の状況で起こるバグの場合、ステップ実行モードでその状況を引き起こすところまで進めるのは難しいので……。

とはいえそれなりの苦労があるからこそ、動いたときの感動は素晴らしいものでした。エミュレータはその性質上、読み込ませるROMによって多彩な様相を見せてくれます。マリオのように単純な横スクロールアクションからポケモンのような大ボリュームのRPGまで、自分が数ヶ月書いた程度のコードの上で動いてしまうわけなのですが、何だか自分が実際以上に凄いことをしたような気分になります（ROMの開発者は偉大ですね）。

決して初心者でも簡単に作れるとは言いませんが、興味のある方は是非挑戦してみてはいかがでしょうか。

さて、「じゃあ作ってみるか～」となった人のために簡単な解説を以下に記載します。

## ゲームボーイエミュレータを書きたい人のためのガイド

先述の通り、難しいアルゴリズムはほぼ存在しないのでAtCoderで茶か緑くらいの実力があれば問題なく処理は書いていけると思います。重要なのは正しい情報を得ることなので、そこを重点的に説明します。

### 一番最初に読むやつ

[Game Boyについての有名な解説動画](https://media.ccc.de/v/33c3-8029-the_ultimate_game_boy_talk)

すっごい有名な動画。大体これで概略は掴めるはず。

[もう一つの基礎的な動画](https://www.youtube.com/playlist?list=PLu3xpmdUP-GRDp8tknpXC_Y4RUQtMMqEu)

こちらはCPUやアセンブラについてなど、より基礎的な情報をカバーしてくれる動画。低レイヤの知識があまりない人はまずこちらから見ることをおすすめします。

### 具体的な仕様を理解する

上で全体的な雰囲気（CPUとPPUがあって、RAMがあって～というような情報）を一通り掴めば、次は[PanDocs](https://gbdev.io/pandocs/)というサイトを中心に見ていきましょう。

メモリマップ、CPU、画面描画、音声出力、コントローラー、割り込み処理等必要なユニットについての詳細が書かれています。「このユニットってどうなってるんだっけ？」と思ったらとりあえず見に行くと良いです。少々簡潔に過ぎる面があるので詳細な情報を得るためには不向きですが、一番最初に参照する場所になると思います。

### CPUの仕様

上記の資料を読んで理解出来た人には説明不要ですが、ゲームボーイのCPUは

1. ゲームROMから命令を読み出す
2. 命令を実行する
3. 次の命令を読み出す

以上の手順を機械的にひたすら繰り返すだけのものです。

各レジスタの仕様等は、先述のPanDocsや[この神PDF](http://marc.rawer.de/Gameboy/Docs/GBCPUman.pdf)の61ページからを読むと良いです。とくにPDFはおすすめで、CPUの全てが書かれています。

#### CPUの命令について

[この神PDF](http://marc.rawer.de/Gameboy/Docs/GBCPUman.pdf)の65ページ目からを読んで実装しましょう。全てがそこにあります。

あとは[命令早見表](https://izik1.github.io/gbops/)も見ると良いです。

命令は全部で500個ほどあり、それら一つ一つを温かみのある手作業で実装していくのはかなり嫌な気持ちになりますが、そこはまあ頑張る感じで......。  
実際のところ、操作する対象のレジスタが異なるだけで行う動作は同じであることが多い（例えばAレジスタの値をBレジスタに代入する命令と、Aレジスタの値をCレジスタに代入する命令は異なるものとして扱われている）ので、ちょっと工夫すればそれなりに実装量を減らすことが出来ます。

1. その命令で行う動作は何か
2. その命令の長さは何byteか
3. その命令に何cycleかかるか
4. 命令後にFレジスタを操作する必要があるか

以上の四点に気を付けながら実装していきましょう。

「この命令では何をすれば良いのか読み取れない」「T-CycleとM-Cycleって何？」といった疑問が出てくることもあると思いますが、そんな時はZ80というCPUのマニュアルを読めば大体解決します。
というのもゲームボーイのCPUはZ80とintel8080を足して二で割ったようなアーキテクチャであり、かつそれらは現在でも愛好家が多いCPUであるため、日本語のドキュメントも非常に充実しています。

私は[このサイト](http://www.yamamo10.jp/yamamoto/comp/Z80/instructions/index.php)にお世話になりました。T-CycleとM-Cycleについても[ここ](http://www.yamamo10.jp/yamamoto/comp/Z80/Z80_Timming/index.php)にばっちり書かれているので、一度読んでみると分かりやすいです。

なお、以下では断りなく「サイクル」と記述した場合、T-Cycleのことであるとします。

#### clock cycleについて

ドキュメントにあるCPUの命令表を見ていると `Clock cycles` という欄があります。これはその命令を実行するのに何回のクロックサイクルを必要とするかを示した値です。
当たり前のことなのですが、普通我々が利用しているPCに搭載されたCPUの計算速度はゲームボーイに搭載されていたそれよりも遥かに高速です。これをそのまま動作させると、例えば画面描画などが異常更新されて大変なことになります。少なくとも明らかに正常な動作はしないように思えますね。

この問題を解決するため、各命令のサイクル数をもとにエミュレータの動作速度を実機のそれと同期させます。

具体的な話をしましょう。普通の――SGBやGBCなどではない――ゲームボーイに搭載されているCPUのClock cycleは`4.194304MHz`です。単位をヘルツにすると`4194304Hz`ですね。つまり一秒間に4194304回のサイクルを実行することが出来ます。

一方で、ゲームボーイのFPS――frames per second, 一秒間に画面を更新する回数――は`59.7275Hz`です。

さて、計算をしましょう。`4194304 // 59.72 = 70224`です。つまり一秒間に六十回の頻度で70224サイクルを処理し、画面描画を実行してあげれば良いということになります。（これは実際のところ順序が逆で、PPUが1フレームを描画するのに70224サイクルかかるため、FPSが59.7275Hzになっています）

定期実行ということでjsのsetInterval概念を使った疑似コードを以下に記述しました。

```js
setInterval(() => {
    const maxCycle = 70224;
    let currentCycle = 0;

    while (currentCycle < maxCycle) {
        // 70224サイクルを処理する
        const cycle = getNextOperationCycle();
        currentCycle += cycle;
        // 色々と処理を実行
    }

    // 画面を描画する
    renderScreen();
}, 1000 / 60)

```

実機に近い動作をさせるには、このような工夫を実装することが必要になります。この話は各ユニットの同期実行のところで詳しく説明します。

#### CPUで踏んだ罠とか気を付けることとか

1. CPU命令で`(HL)`みたいに括弧でくくられている場合、括弧でくくられているレジスタが格納する値をメモリアドレスとして扱い、そのアドレスに格納されている値を扱うということです。
2. 基本的にリトルエンディアンなので、low -> highの順で読み書きするよう実装しましょう
3. stackはアドレスが大きい方から小さい方へと伸びていきます。pushするときはSPをデクリメントし、popするときはSPをインクリメントしましょう
4. 純粋に500命令近く書くのが面倒。本当に。面倒です。かといってYoutubeとか見ながらやると注意散漫になってミスるので気を付けてください
5. half carryってなに？って思ったけど、3bit目で繰り上がりが起きているかどうかを調べればよい -> `(0x0F & left) + (0x0F & right) == 0x10` をします。普通のcarryも同じ要領で。16bit版も同じ
6. DAA命令って何？ -> [二進化十進表現(BCD)](https://ja.wikipedia.org/wiki/%E4%BA%8C%E9%80%B2%E5%8C%96%E5%8D%81%E9%80%B2%E8%A1%A8%E7%8F%BE)というおもしろ表現があるので、Aレジスタに入っている二進数の値をそれに変換します。なおNフラグが立っているかどうか、Cフラグが立っているかどうか、Hフラグが立っているかどうかで場合分けがつらいことになるので、[参考資料](https://forums.nesdev.org/viewtopic.php?t=15944)を見てやるのがおすすめ。あと[ここ](https://www.tech-diningyo.info/entry/2021/08/20/222721)も役に立った
7. STOP命令が2バイトになっている資料がたまに存在するが、どうもこれはGB実機側のバグで、STOPの後にNOPを入れなければ実機では動作しなかったらしい。どうせ00はNOPなので先頭1バイトだけ読めばよい
8. RLCA命令とRLA命令って何が違うんですか？ -> [Z80命令セットの日本語解説](http://www.yamamo10.jp/yamamoto/comp/Z80/instructions/index.php)があるので読む。ネットの海にはなんでもある
9. CALLってどういう処理書くんですか？ -> [ここ](https://gbdev.io/pandocs/CPU_Instruction_Set.html)を見る

### メモリマップとROMとRAM

大体[Pan Docs](https://gbdev.io/pandocs/Memory_Map.html)に全てが書いてあります。

前提として、Game Boyには8KiBのRAMと8KiBのVRAMが搭載されています（これが2003年まで製造販売されてたってマジ？）。 翌年発売のDSは4MB RAMを搭載していることを考えるとまあすごいですね。

Game Boyに搭載されているCPUには16bitのメモリバスが存在するので、64KiBまでのメモリアドレスを扱うことができます。RAM 8KiB + VRAM 8KiB = 16KiB なので、64KiB - 16KiB = 48KiB余りますね。その余った分をROMの読み込みやデバイスアクセス、スプライトの管理等に使用します。

以下は詳細なメモリマップです。[Pan Docs](https://gbdev.io/pandocs/Memory_Map.html)から引っ張ってきました。

| 開始位置  | 終了位置 | 内容                            | 備考                                                                  |
| -------- | -------- | ------------------------------ | --------------------------------------------------------------------- |
| 0000     | 3FFF     | 16 KiB ROM Bank 00              | バンク0(カートリッジ最初の16KiB)で固定                                 |
| 4000     | 7FFF     | 16 KiB ROM Bank 01~NN          | MBCによってバンクを切り替え可能                                       |
| 8000     | 9FFF     | 8 KiB Video RAM (VRAM)         | 画面表示に使用。ゲームボーイカラーではバンク切り替え可能              |
| A000     | BFFF     | 8 KiB External RAM             | カートリッジに増設されたRAMをコントロールする領域。バンク切り替え可能 |
| C000     | CFFF     | 4 KiB Work RAM (WRAM)          | メインメモリ                                                          |
| D000     | DFFF     | 4 KiB Work RAM (WRAM)          | メインメモリ。ゲームボーイカラーではバンク切り替え可能                |
| E000     | FDFF     | C000~DDFFのミラー (ECHO RAM)   | C000 から DDFF を読み込んだ時と同じ内容が出現する                     |
| FE00     | FE9F     | Sprite attribute table (OAM)   | スプライトの管理を行う領域                                            |
| FEA0     | FEFF     | Not Usable                     | 使用が禁止されている領域                                      |
| FF00     | FF7F     | I/O Registers                  | デバイスアクセス時に使用                                              |
| FF80     | FFFE     | High RAM (HRAM)                | スタック用の領域                                                      |
| FFFF     | FFFF     | Interrupt Enable register (IE) | 割り込みの有効/無効を切り替える                                        |

今回注目するべきなのは上二つです。そもそもROMにアクセスするアドレスが32KiBしか割り当てられていません。これは極めて狭い範囲です。  
ROM容量が32KiB以内に収まるのはテトリスのような極めてシンプルなゲームくらいで、ほとんどのゲームは32KiBを超過してしまいます。

例えばポケモン赤緑の国内版ROMの容量は512KiBですし、海外版に至ってはちょうど1MiBだそうです。どうやったってメモリバスが足りません。  
素直にやれば、最初の32KiB以降のROMの中身にアクセスすることは出来ないということになります。これはとても困りますね。

それを解決するための工夫がMBCs, Memory Bank Controllersです。

MBCは以下のように動作します。

1. 大きなROMを16KiBごとの区画に切り分ける
2. その区画に対し、前から0-indexedで番号を割り振る
3. 番号を指定すると、その番号が振られた区画のデータがMBCによって0x4000 ~ 0x7FFFの範囲にマッピングされる

この機能を使用することで、ROM上の任意の場所にあるデータを読み込むことが出来るようになります。

ところでMBCsは本来ROMカートリッジ内に実装されているチップで、厳密にいえばゲームボーイの本体そのものには存在しないチップなのですが、エミュレータは物理カートリッジを扱わないので当然実装する必要があります。

### PPUの実装

鬼門です。ここを乗り越えれば画面描画が可能になり一気にエミュレータらしくなってきます。進捗が可視化されて楽しいパートなのですが、何を実装すべきか分からずに辛くなるパートにもなりやすいので、私がどう乗り越えたかを書いておきます。

まず見るのは例によってPandocですが、PPUについてはまた素晴らしい資料があります。[GBEDG](http://pixelbits.16-b.it/GBEDG/ppu/)です。大体ここにPPU単体の動作について全てが書かれています。GBEDGで図解されている内容も多いですが、一応軽い解説を下に書きます。

#### タイル

Game Boyは原則、8×8ピクセルのタイルが描画の最小単位らしいです。要は1ピクセルに対して個別に描画設定をすることはできず、8×8のグリッドを指定して、そこに前もってROM側で用意したテクスチャを貼り付けていくイメージですかね。

タイルデータは一つにつき16byteで表現され、VRAMの`$8000~$97FF`に保存されます。つまり最大で384種類のタイルを格納できることになります。

#### レイヤー

BG, Window, Objectの三種類のレイヤーがあります。BGはback groundの略で背景を意味し、WindowはBGの上に覆いかぶせることの出来るレイヤです。双方ともに8×8のタイルで描画されます。

Objectはいわゆるスプライトで、8×8のタイルを無視して自由に置くことの出来るピクセルです。ただし置ける個数などの制約があります。分かりやすい図解がGBEDGにあるのでそれを見ましょう。

#### BG

`32*32`のタイルを敷き詰めることが出来る描画領域です。ピクセル数にすると8倍なので`256*256`ピクセルで描画します。  
Game Boyにおける実際の画面の大きさは`20*18`タイル、`160*144`ピクセルとなっており、BGの一部を切り取る形で表示します。広いBG領域を切り取る窓のようなイメージです(それ故にview portと呼ばれています)。実際にどの領域を表示するかはPPU上のSCYレジスタとSCXレジスタに代入された値によって決まります。

分かりやすい図解がGBEDGにあるので以下略。

描画するタイルの指定は`$9800~$9BFF`か`$9C00~$9FFF`のどちらかに指定します。どちらのタイルマップを利用するかは`$FF40`に確保されているLCD Controlの3bitまたは6bit目が立っているかどうかで切り替えられます。

view portのピクセルデータをバッファとして保持し、画面描画系のライブラリに食わせてあげるのが実装として丸いと思います。

#### CPUとの同期とPPU描画モード

CPUとPPUは同期して動かす必要がありますが、各々が並列で動いているということを忘れてはいけません。分かりやすく言えば、CPUがPPUを動かす命令を出したりしているわけではないということです。ではどのように同期させるのかですが、CPUのところでも出て来たサイクルの概念をここで使います。

実は「CPUがNサイクル進んだ場合、PPUは～～の動作を行う」ということが仕様として決まっています。それが「PPU描画モード」という概念です。

PPUでは、view portに対してピクセルデータを1行描画するのに456サイクルかかります。この456サイクルの中で三つのモードが切り替わります。