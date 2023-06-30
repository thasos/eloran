# Eloran

Comics and Ebook web library written in rust, with reading, search, reading status, bookmarks...

## Intro

I used [Ubooquity](https://vaemendis.net/ubooquity/) during a few years, but unfortunatly it is not opensource, and there is no read status, so I decided to find another solution.

I tried some alternatives :
- [Komga](https://komga.org) : best project here I think, but no "folder view" 😥
- [Tanoshi](https://github.com/faldez/tanoshi) : works well, in rust too 🦀🚀, but no support for ebooks
- [Calibre web](https://github.com/janeczku/calibre-web) or [BicBucStriim](https://github.com/rvolz/BicBucStriim) : I just can't use Calibre's classification system
- [Kavita](https://github.com/Kareadita/Kavita) : nice project too, but I don't like the Collections system (feels like Calibre)
- [Nextcloud epubreader](https://apps.nextcloud.com/apps/epubreader) : an old app, but it doesn't work with most of my collection

So here I am, a personal project named after my childrens ([Elora](https://en.wikipedia.org/wiki/Elora_Danan) and [Revan](https://en.wikipedia.org/wiki/Revan)).

Feel free to use, improve, and cry to my low code quality !

- use a sqlite database
- store ebooks and comcis covers in database (~10ko per cover, almost 160 Mo for 15000 files)
- multiple users, with bookmarks pages, reading status with page number (not for pdf)
- periodic scan of libraries folders
- no cached data, comics images are extracted on the fly
- comics pages responsive size for optimized mobile network usage
- small binary : 5 Mio, alpine based image : 13 Mio
- small css, small compressed svg, no javascript
- rust 🦀🚀

## Screenshots

I know this is AWFUL 🤮, I have not worked on the css yet, please be patient (or help meeeee 🆘) !

![grid view](./doc/grid.png) ![file info](./doc/info.png) ![reading](./doc/reading.png)

## Installation

### Podman / Docker

Feel free to customize listen port and path...

```
podman pull ghcr.io/thasos/eloran:latest
podman run -d -p 0.0.0.0:3200:3200 \
    -v /host_data/eloran/sqlite:/opt/eloran/sqlite \
    -v /host_data/library:/library \
    --name eloran \
    ghcr.io/thasos/eloran:0.1.1
```

### From source

For now you need the `css` directory, so the simpliest way is to clone sources and build it with cargo, a usable binary and docker image will be available soon.

```
git clone https://github.com/thasos/eloran.git
cd eloran
just build
target/x86_64-unknown-linux-gnu/release/eloran
```

If you do note use [just](https://github.com/casey/just), use it 😁 or just launch `cargo build --release`

### Build dependencies

Arch :
```
sudo pacman -S libarchive cairo poppler-glib
# if you want to package it in alpine image
sudo pacman -S musl
```

Debian/ubuntu :
```
sudo apt install libarchive-dev libcairo2-dev libpoppler-glib-dev
# if you want to package it in alpine image
sudo apt install musl-dev
```

Fedora :
```
sudo dnf install rust-glib-sys-devel.noarch
sudo dnf install rust-cairo-sys-rs0.16-devel.noarch
sudo dnf install cairo-gobject-devel.x86_64
sudo dnf install poppler-glib-devel.x86_64
sudo dnf install libarchive-devel.x86_64
```

## TODO

- [ ] hash password in database !!
- [ ] store session in database (see `fn create_router()` in [src/http_server.rs](http_server.rs))
- [ ] pretty error handling
- [ ] more testing
- [ ] allow relative path in `library_path`
- [ ] fix element numbers for sub directories
- [ ] handle `cover.jpg` files for directories (or use first file's cover ?)
- [ ] customized css
- [ ] upload files
- [ ] install page at 1st start : admin password, library_path, new user...
- [ ] share files (or directories, or page)
- [ ] grid or list view in preferences
- [ ] progress bar while reading, file info, and grid view
- [ ] easy go to page number while reading and from file info
- [ ] read pdf in new tab
- [ ] display read status in bookmarks page
- [ ] better css 🤪
- [ ] true ebook reading
- [ ] export read status
- [ ] list "next to read"
