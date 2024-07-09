# Bash's TF2 Monitor

A successor to my previous project [TF2 Bot Kicker GUI](https://github.com/Bash-09/tf2-bot-kicker-gui), which was an alternative to [TF2 Bot Detector](https://github.com/PazerOP/tf2_bot_detector), which is a no-longer maintained program to automatically kick bots and cheaters from the game.

TF2 Monitor does not have quite the same goals as those previous projects, and while it does have a feature to automatically call votekicks on accounts marked as Bots, the purpose is not just for the automation of removing bad actors from the game. Instead, it is simply a desktop app designed to visualise information about the TF2 matches you are in and record some data on players of interest.

If you'd like to try it out yourself, check out the **Setup** section below.

![image](https://github.com/Bash-09/MAC-Desktop/assets/47521168/cda83c78-e1a4-4a81-b54f-d90ac50cfda6)

# Features
- View all the players currently in your match of TF2
  - See details of interest about each player at-a-glance
    - Which verdict you have assigned them (or `Player` by default)
    - Presence of VAC or Game bans
    - Profile visibility (i.e. Private or Friends Only)
    - If the account is less than a few months old
    - Which players are friends with each other on the server (depending on friend list privacy settings of players)
    - Which players are *your* friends! (Again depending on friend list privacy of your or their account)
    - Any notes you have attached to a player (on hover of the notes icon)
    - More???
  - Annotate players with a Verdict, `Trusted`, `Player`, `Suspicious`, `Cheater`, or `Bot` to keep track of them in future games
  - Attach private notes to players to record any interesting information you might want to remember about them
- View more specific information about a player
  - Game information such as their team, ping and k/d
  - Steam Profile information (including all the at-a-glance information mentioned above, but in more detail)
- View the chat and killfeed history of the match
  - Click on the player's name to view more about their account!
- Review the all the players who currently have any information recorded
  - Filtering options such as whitelisting specific verdicts, or searching for a steamid, name, or note attached to a player
- Compatible with the [MAC Client](https://github.com/MegaAntiCheat/client-backend), allowing you to contribute to demo collection using this client

![image](https://github.com/Bash-09/MAC-Desktop/assets/47521168/12fc2fb6-ada5-4fa4-bdbf-28d52b6f4d08)

# Setup
1. Download one of the releases or build the app yourself.
2. Add `-usercon -condebug -conclearlog -g15` to your TF2 launch options (Right click Team Fortress 2 in your Steam library -> Properties -> Paste into the "Launch Options" input field)
3. Add the following to your `autoexec.cfg` file (you may need to [create your autoexec](https://steamcommunity.com/sharedfiles/filedetails/?id=3112357964) in the first place)
  - If you use mastercomfig, you will need to place your autoexec file inside the `overrides` folder inside your `cfg` folder (if `overrides` doesn't exist, just create it)
```
ip 0.0.0.0
rcon_password tf2monitor
net_start
```
4. Start TF2
5. Start the application
6. Open the settings window and put in your [Steam Web API key](https://steamcommunity.com/dev/apikey) to enable steam profile lookups.

You can change the password used for rcon if you like (instead of `tf2monitor`), but you will also have to change it in the settings panel of the app. Rcon should never be accessible to anything outside of your computer unless you explicitly configure it to be, so security is not a major concern when choosing the rcon password, it is simply required to be set.

## Troubleshooting

- **Rcon connection error in the console window**
  - Ensure your `autoexec.cfg` is executing properly by doing the following
  - Check step 3 of **Setup** if you use Mastercomfig
  - Make sure you haven't accidentally created `autoexec.cfg.txt` (you might need to [show file extensions](https://www.howtogeek.com/205086/beginner-how-to-make-windows-show-file-extensions/) to tell)
  - When you launch TF2, open your console and look for a line that looks like `Network: IP 0.0.0.0, mode MP, dedicated No, ports 27015 SV / 27005 CL`
    - If you see this line, `autoexec.cfg` is probably being executed
    - If you do not have this line in your console, your `autoexec.cfg` is probably not being executed
  - Restart TF2, then paste the commands `ip 0.0.0.0`, `rcon_password tf2monitor` and `net_start` into your console manually
    - Restart the app and see if it can connect afterwards
    - If it successfully connects after that, your `autoexec.cfg` file is not executing
  - Another program may be using that port, you can try change the rcon port that the app will use
    - This is common if you have installed iTunes before
    1. Add`-port 27069` to your TF2 launch options (or choose another suitable and available port number)
    2. In the settings window, change the `Rcon port` number to `27069`
    3. Restart TF2 and then the app
- **RCon authentication error**
  - Your rcon password is not being accepted
  - Try change the password you set in your `autoexec.cfg` and update it in the settings window
  - Choose a password that does not contain any spaces or special characters, a single word is fine
  - The password is required to be set but does not have to be secure as nobody else will have access to your Rcon
- **No players show up in the UI when I join a match**
  - Check for an Rcon connection error in the console window and follow the steps above
- **Missing launch options, but I'm not**
  - Run the application with the flag `--ignore-lanch-opts`
- **Can't locate TF2 directory**
  - Use the command suggested in the error
- **Can't locate Steam directory**
  - If you have previously installed Steam via Flatpak, ensure there are no residual files
  - May be in `~/.var/app/`
  - Launch the app via the command line with the arguments `--steam-user 7565... --tf2-dir path_to_your_tf2_installation`, substituting your SteamID and TF2 installation location
- **I can't get an API key because I haven't bought any games**
  - Only premium steam accounts (an account that has purchased something on it) can be issued Steam Web API keys
  - You do not have to provide a steam API key, the app will still provide most functionality but will not have some features (such as looking up the steam profiles of players)

# Technical info
The UI is built using the [iced](https://github.com/iced-rs/iced) GUI library using Rust.

Interaction with TF2 happens via [RCON](https://developer.valvesoftware.com/wiki/Source_RCON_Protocol), which allows an external program to establish a network connection and issue console commands to the game remotely (as long as the user has the `-usercon` launch option set). Other console information is read back from the `console.log` file which TF2 writes all the contents of the in-game console into in real-time (as long as the user has the `-condebug` launch option set).

# Building
Building requires [Rust to be installed](https://www.rust-lang.org/tools/install), then simply run `cargo run --release` from inside the repository (Some dependencies may need to be installed on Linux).

On some platforms you may need to instll some additional dependencies, e.g. on Ubuntu, you will have to install `libssl-dev`.

