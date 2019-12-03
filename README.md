# wpbrute-rs
High performance WordPress login bruteforcer with automatic concurrency for maximum amount of tries per second.

# How to install?

* Install Rust with https://rustup.rs/ or your system's package repositories.
* Run `$ cargo install --force --git https://github.com/leo-lb/wpbrute-rs.git` (You can run this again in the future to update!)

NOTE: minimum required Rust version is 1.39.

# How to use?

```
wpbrute-rs 0.1.0
Leo Le Bouter <lle-bout@zaclys.net>

USAGE:
    wpbrute-rs -w <password-list> -t <target-wp-login> -a <user-agent> -u <username>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -w <password-list>          
    -t <target-wp-login>        
    -a <user-agent>              [default: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like
                                Gecko) Chrome/77.0.3865.120 Safari/537.36]
    -u <username>                [default: admin]
```

# Example

`$ wpbrute-rs -w /usr/share/wordlists/rockyou.txt -t http://wordpress.example.local/wp-login.php`