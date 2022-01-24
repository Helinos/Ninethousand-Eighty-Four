# Ninethousand-Eighty-Four
An amateur implementation of [Randal Munroe's r9k system](https://blog.xkcd.com/2008/01/14/robot9000-and-xkcd-signal-attacking-noise-in-chat/) for discord, inspired by [Signal](https://github.com/Caltrop256/signal-discord-r9k-bot/) and written entirely in rust.

### Implementation
#### Text
When considering the content of the message, the bot will filter out any character aside from `A-z` (Case insensitive) `0-9` and trim anything enclosed by `<>` angle brackets.
In the raw message content sent to a discord bot, all channel/user mentions and emojis get enclosed by angle brackets.
Practically, this means that the bot ignores all punctuation, whitespace, mentions, and emojis, which means messages like `Yeah! Sure thing! @User` and `yeah sure thing. 😀` are both recorded as `yeahsurething`. 
##### Attachments
Attachments are currently not considered by the bot when gauging originality.
For now, one will have to include a message with any images sent.
#### Mutes
After a user sends a violating message, the user's streak will be incremented by 1 and will subsequently get muted for `2^(2 * streak - 1)` seconds.
This mute's the user for 2 seconds and quadruples the duration for every subsequent violation.
The user's streak will decay by 1 every 6 hours until it is back to 0.

## Why was I muted?
(For those who are not knowers)

I recommend reading above, but to state briefly, this bot compares messages that get said in guilds against a list of every other message that's been said prior. If the bot decides a message is similar enough to a message that has been sent in the past, or in other words, if the message is unoriginal, then it deletes the message and mutes the user for increasingly long durations.
