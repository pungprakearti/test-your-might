// Signs files the way the release workflow does, for testing self-updates
// (tools/update-e2e.sh). Not part of the game.
//
//   cargo run --example update_sign -- keygen <dir>
//       writes <dir>/test.key (unencrypted secret key) and <dir>/test.pub
//       (base64 public key line, for TYM_UPDATE_PUBKEY)
//   cargo run --example update_sign -- sign <secret key> <file> <trusted comment>
//       writes <file>.minisig

use std::fs;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["keygen", dir] => {
            let pair = minisign::KeyPair::generate_unencrypted_keypair().expect("generate key");
            let dir = Path::new(dir);
            let secret = pair.sk.to_box(None).expect("encode secret key").into_string();
            fs::write(dir.join("test.key"), secret).expect("write test.key");
            fs::write(dir.join("test.pub"), pair.pk.to_base64()).expect("write test.pub");
        }
        ["sign", key, file, comment] => {
            let sk_box = minisign::SecretKeyBox::from_string(&fs::read_to_string(key).expect("read secret key"))
                .expect("parse secret key");
            let sk = minisign::SecretKey::from_unencrypted_box(sk_box).expect("unencrypted secret key");
            let data = fs::File::open(file).expect("open file to sign");
            let sig = minisign::sign(None, &sk, data, Some(comment), None).expect("sign");
            fs::write(format!("{file}.minisig"), sig.to_string()).expect("write signature");
        }
        _ => {
            eprintln!("usage: update_sign keygen <dir> | sign <secret key> <file> <trusted comment>");
            std::process::exit(2);
        }
    }
}
