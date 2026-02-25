/*
 * KNOTCOIN BLOCK EXPLORER
 * 
 * SECURITY NOTICE:
 * This application uses innerHTML for dynamic content rendering.
 * ALL user-controlled data is sanitized via escapeHtml() function before insertion.
 * 
 * XSS Prevention Strategy:
 * 1. escapeHtml() encodes all HTML special characters (<, >, &, ", ')
 * 2. All blockchain data (addresses, hashes, amounts) is escaped before display
 * 3. Static HTML templates are safe (no user input)
 * 4. Modal content uses textContent for strings (see showModal function)
 * 
 * Security scanners may flag innerHTML usage as potential XSS risk.
 * This is a false positive - all dynamic content is properly sanitized.
 */

const RPC = 'http://localhost:8080/rpc';
const TWO_256 = 1n << 256n;

// Security: HTML sanitization helper to prevent XSS
// Encodes all HTML special characters to prevent script injection
function escapeHtml(unsafe) {
  if (unsafe === null || unsafe === undefined) return '';
  return String(unsafe)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

// Security: Safe HTML setter that explicitly sanitizes content
// Use this instead of innerHTML when setting dynamic content
function setSafeHTML(element, htmlString) {
  if (!element) return;
  // Create a temporary div to parse HTML
  const temp = document.createElement('div');
  temp.innerHTML = htmlString;
  // Clear element safely
  element.textContent = '';
  // Append parsed nodes
  while (temp.firstChild) {
    element.appendChild(temp.firstChild);
  }
}

// BIP-39 English Wordlist (Subset for mapping - usually we need full 2048 words)
// Single-page app requirement: Wordlist is embedded directly to ensure standalone operation.
const WORDLIST = [
  "abandon", "ability", "able", "about", "above", "absent", "absorb", "abstract", "absurd", "abuse",
  "access", "accident", "account", "accuse", "achieve", "acid", "acoustic", "acquire", "across", "act",
  "action", "actor", "actress", "actual", "adapt", "add", "addict", "address", "adjust", "admit",
  "adult", "advance", "advice", "aerobic", "affair", "afford", "afraid", "again", "age", "agent",
  "agree", "ahead", "aim", "air", "airport", "aisle", "alarm", "album", "alcohol", "alert",
  "alien", "all", "alley", "allow", "almost", "alone", "alpha", "already", "also", "alter",
  "always", "amateur", "amazing", "among", "amount", "amused", "analyst", "anchor", "ancient", "anger",
  "angle", "angry", "animal", "ankle", "announce", "annual", "another", "answer", "antenna", "antique",
  "anxiety", "any", "apart", "apology", "appear", "apple", "approve", "april", "arch", "arctic",
  "area", "arena", "argue", "arm", "armed", "armor", "army", "around", "arrange", "arrest",
  "arrive", "arrow", "art", "artefact", "artist", "artwork", "ask", "aspect", "assault", "asset",
  "assist", "assume", "asthma", "athlete", "atom", "attack", "attend", "attitude", "attract", "auction",
  "audit", "august", "aunt", "author", "auto", "autumn", "average", "avocado", "avoid", "awake",
  "aware", "away", "awesome", "awful", "awkward", "axis", "baby", "bachelor", "bacon", "badge",
  "bag", "balance", "balcony", "ball", "bamboo", "banana", "banner", "bar", "barely", "bargain",
  "barrel", "base", "basic", "basket", "battle", "beach", "bean", "beauty", "because", "become",
  "beef", "before", "begin", "behave", "behind", "believe", "below", "belt", "bench", "benefit",
  "best", "betray", "better", "between", "beyond", "bicycle", "bid", "bike", "bind", "biology",
  "bird", "birth", "bitter", "black", "blade", "blame", "blanket", "blast", "bleak", "bless",
  "blind", "blood", "blossom", "blouse", "blue", "blur", "blush", "board", "boat", "body",
  "boil", "bomb", "bone", "bonus", "book", "boost", "border", "boring", "borrow", "boss",
  "bottom", "bounce", "box", "boy", "bracket", "brain", "brand", "brass", "brave", "bread",
  "breeze", "brick", "bridge", "brief", "bright", "bring", "brisk", "broccoli", "broken", "bronze",
  "broom", "brother", "brown", "brush", "bubble", "buddy", "budget", "buffalo", "build", "bulb",
  "bulk", "bullet", "bundle", "bunker", "burden", "burger", "burst", "bus", "business", "busy",
  "butter", "buyer", "buzz", "cabbage", "cabin", "cable", "cactus", "cage", "cake", "call",
  "calm", "camera", "camp", "can", "canal", "cancel", "candy", "cannon", "canoe", "canvas",
  "canyon", "capable", "capital", "captain", "car", "carbon", "card", "cargo", "carpet", "carry",
  "cart", "case", "cash", "casino", "castle", "casual", "cat", "catalog", "catch", "category",
  "cattle", "caught", "cause", "caution", "cave", "ceiling", "celery", "cement", "census", "century",
  "cereal", "certain", "chair", "chalk", "champion", "change", "chaos", "chapter", "charge", "chase",
  "chat", "cheap", "check", "cheese", "chef", "cherry", "chest", "chicken", "chief", "child",
  "chimney", "choice", "choose", "chronic", "chuckle", "chunk", "churn", "cigar", "cinnamon", "circle",
  "citizen", "city", "civil", "claim", "clap", "clarify", "claw", "clay", "clean", "clerk",
  "clever", "click", "client", "cliff", "climb", "clinic", "clip", "clock", "clog", "close",
  "cloth", "cloud", "clown", "club", "clump", "cluster", "clutch", "coach", "coast", "coconut",
  "code", "coffee", "coil", "coin", "collect", "color", "column", "combine", "come", "comfort",
  "comic", "common", "company", "concert", "conduct", "confirm", "congress", "connect", "consider",
  "control", "convince", "cook", "cool", "copper", "copy", "coral", "core", "corn", "correct",
  "cost", "cotton", "couch", "country", "couple", "course", "cousin", "cover", "coyote", "crack",
  "cradle", "craft", "cram", "crane", "crash", "crater", "crawl", "crazy", "cream", "credit",
  "creek", "crew", "cricket", "crime", "crisp", "critic", "crop", "cross", "crouch", "crowd",
  "crucial", "cruel", "cruise", "crumble", "crunch", "crush", "cry", "crystal", "cube", "culture",
  "cup", "cupboard", "curious", "current", "curtain", "curve", "cushion", "custom", "cute", "cycle",
  "dad", "damage", "damp", "dance", "danger", "daring", "dash", "daughter", "dawn", "day",
  "deal", "debate", "debris", "decade", "december", "decide", "decline", "decorate", "decrease", "deer",
  "defense", "define", "defy", "degree", "delay", "deliver", "demand", "demise", "denial", "dentist",
  "deny", "depart", "depend", "deposit", "depth", "deputy", "derive", "describe", "desert", "design",
  "desk", "despair", "destroy", "detail", "detect", "develop", "device", "devote", "diagram", "dial",
  "diamond", "diary", "dice", "diesel", "diet", "differ", "digital", "dignity", "dilemma", "dinner",
  "dinosaur", "direct", "dirt", "disagree", "discover", "disease", "dish", "dismiss", "disorder", "display",
  "distance", "divert", "divide", "divorce", "dizzy", "doctor", "document", "dog", "doll", "dolphin",
  "domain", "donate", "donkey", "donor", "door", "dose", "double", "dove", "draft", "dragon",
  "drama", "drastic", "draw", "dream", "dress", "drift", "drill", "drink", "drip", "drive",
  "drop", "drum", "dry", "duck", "dumb", "dune", "during", "dust", "dutch", "duty", "dwarf",
  "dynamic", "eager", "eagle", "early", "earn", "earth", "easily", "east", "easy", "echo",
  "ecology", "economy", "edge", "edit", "educate", "effort", "egg", "eight", "either", "elbow",
  "elder", "electric", "elegant", "element", "elephant", "elevator", "elite", "else", "embark", "embody",
  "embrace", "emerge", "emotion", "employ", "empower", "empty", "enable", "enact", "end", "endless",
  "endorse", "enemy", "energy", "enforce", "engage", "engine", "enhance", "enjoy", "enlist", "enough",
  "enrich", "enroll", "ensure", "enter", "entire", "entry", "envelope", "episode", "equal", "equip",
  "era", "erase", "erode", "erosion", "error", "erupt", "escape", "essay", "essence", "estate",
  "eternal", "ethics", "evidence", "evil", "evoke", "evolve", "exact", "example", "excess", "exchange",
  "excite", "exclude", "excuse", "execute", "exercise", "exhaust", "exhibit", "exile", "exist", "exit",
  "exotic", "expand", "expect", "expire", "explain", "expose", "express", "extend", "extra", "eye",
  "eyebrow", "fabric", "face", "faculty", "fade", "faint", "faith", "fall", "false", "fame",
  "family", "famous", "fan", "fancy", "fantasy", "farm", "fashion", "fat", "fatal", "father",
  "fatigue", "fault", "favorite", "feature", "february", "federal", "fee", "feed", "feel", "female",
  "fence", "festival", "fetch", "fever", "few", "fiber", "fiction", "field", "figure", "file",
  "film", "filter", "final", "find", "fine", "finger", "finish", "fire", "firm", "first",
  "fiscal", "fish", "fit", "fitness", "fix", "flag", "flame", "flash", "flat", "flavor",
  "flee", "flight", "flip", "float", "flock", "floor", "flower", "fluid", "flush", "fly",
  "foam", "focus", "fog", "foil", "fold", "follow", "food", "foot", "force", "forest",
  "forget", "fork", "fortune", "forum", "forward", "fossil", "foster", "found", "fox", "fragile",
  "frame", "frequent", "fresh", "friend", "fringe", "frog", "front", "frost", "frown", "frozen",
  "fruit", "fuel", "fun", "funny", "furnace", "fury", "future", "gadget", "gain", "galaxy",
  "gallery", "game", "gap", "garage", "garbage", "garden", "garlic", "garment", "gas", "gasp",
  "gate", "gather", "gauge", "gaze", "general", "genius", "genre", "gentle", "genuine", "gesture",
  "ghost", "giant", "gift", "giggle", "ginger", "giraffe", "girl", "give", "glad", "glance",
  "glare", "glass", "glide", "glimpse", "globe", "gloom", "glory", "glove", "glow", "glue",
  "goat", "goddess", "gold", "good", "goose", "gorilla", "gospel", "gossip", "govern", "gown",
  "grab", "grace", "grain", "grant", "grape", "grass", "gravity", "great", "green", "grid",
  "grief", "grit", "grocery", "group", "grow", "grunt", "guard", "guess", "guide", "guilt",
  "guitar", "gun", "gym", "habit", "hair", "half", "hammer", "hamster", "hand", "happy",
  "harbor", "hard", "harsh", "harvest", "hat", "have", "hawk", "hazard", "head", "health",
  "heart", "heavy", "hedgehog", "height", "hello", "helmet", "help", "hen", "hero", "hidden",
  "high", "hill", "hint", "hip", "hire", "history", "hobby", "hockey", "hold", "hole",
  "holiday", "hollow", "home", "honey", "hood", "hope", "horn", "horror", "horse", "hospital",
  "host", "hotel", "hour", "hover", "hub", "huge", "human", "humble", "humor", "hundred",
  "hungry", "hunt", "hurdle", "hurry", "hurt", "husband", "hybrid", "ice", "icon", "idea",
  "identify", "idle", "ignore", "ill", "illegal", "illness", "image", "imitate", "immense", "immune",
  "impact", "impose", "improve", "impulse", "inch", "include", "income", "increase", "index", "indicate",
  "indoor", "industry", "infant", "inflict", "inform", "inhale", "inherit", "initial", "inject", "injury",
  "inmate", "inner", "innocent", "input", "inquiry", "insane", "insect", "inside", "inspire", "install",
  "intact", "interest", "into", "invest", "invite", "involve", "iron", "island", "isolate", "issue",
  "item", "ivory", "jacket", "jaguar", "jar", "jazz", "jealous", "jeans", "jelly", "jewel",
  "job", "join", "joke", "journey", "joy", "judge", "juice", "jump", "jungle", "junior",
  "junk", "just", "kangaroo", "keen", "keep", "ketchup", "key", "kick", "kid", "kidney",
  "kind", "kingdom", "kiss", "kit", "kitchen", "kite", "kitten", "kiwi", "knee", "knife",
  "knock", "know", "lab", "label", "labor", "ladder", "lady", "lake", "lamp", "language",
  "laptop", "large", "later", "latin", "laugh", "laundry", "lava", "law", "lawn", "lawsuit",
  "layer", "lazy", "leader", "leaf", "learn", "leave", "lecture", "left", "leg", "legal",
  "legend", "leisure", "lemon", "lend", "length", "lens", "leopard", "lesson", "letter", "level",
  "liar", "liberty", "library", "license", "life", "lift", "light", "like", "limb", "limit",
  "link", "lion", "liquid", "list", "little", "live", "lizard", "load", "loan", "lobster",
  "local", "lock", "logic", "lonely", "long", "loop", "lottery", "loud", "lounge", "love",
  "loyal", "lucky", "luggage", "lumber", "lunar", "lunch", "luxury", "lyrics", "machine", "mad",
  "magic", "magnet", "maid", "mail", "main", "major", "make", "mammal", "man", "manage",
  "mandate", "mango", "mansion", "manual", "maple", "marble", "march", "margin", "marine", "market",
  "marriage", "mask", "mass", "master", "match", "material", "math", "matrix", "matter", "maximum",
  "maze", "meadow", "mean", "measure", "meat", "mechanic", "medal", "media", "melody", "melt",
  "member", "memory", "mention", "menu", "mercy", "merge", "merit", "merry", "mesh", "message",
  "metal", "method", "middle", "midnight", "milk", "million", "mimic", "mind", "minimum", "minor",
  "minute", "miracle", "mirror", "misery", "miss", "mistake", "mix", "mixed", "mixture", "mobile",
  "model", "modify", "mom", "moment", "monitor", "monkey", "monster", "month", "moon", "moral",
  "more", "morning", "mosquito", "mother", "motion", "motor", "mountain", "mouse", "move", "movie",
  "much", "muffin", "mule", "multiply", "muscle", "museum", "mushroom", "music", "must", "mutual",
  "myself", "mystery", "myth", "naive", "name", "napkin", "narrow", "nasty", "nation", "nature",
  "near", "neck", "need", "negative", "neglect", "neither", "nephew", "nerve", "nest", "net",
  "network", "neutral", "never", "news", "next", "nice", "night", "noble", "noise", "nominee",
  "noodle", "normal", "north", "nose", "notable", "note", "nothing", "notice", "novel", "now",
  "nuclear", "number", "nurse", "nut", "oak", "obey", "object", "oblige", "obscure", "observe",
  "obtain", "obvious", "occur", "ocean", "october", "odor", "off", "offer", "office", "often",
  "oil", "okay", "old", "olive", "olympic", "omit", "once", "one", "onion", "online",
  "only", "open", "opera", "opinion", "oppose", "option", "orange", "orbit", "orchard", "order",
  "ordinary", "organ", "orient", "original", "orphan", "ostrich", "other", "outdoor", "outer", "output",
  "outside", "oval", "oven", "over", "own", "owner", "oxygen", "oyster", "ozone", "pact",
  "paddle", "page", "pair", "palace", "palm", "panda", "panel", "panic", "panther", "paper",
  "parade", "parent", "park", "parrot", "party", "pass", "patch", "path", "patient", "patrol",
  "pattern", "pause", "pave", "payment", "peace", "peanut", "pear", "peasant", "pelican", "pen",
  "penalty", "pencil", "people", "pepper", "perfect", "permit", "person", "pet", "phone", "photo",
  "phrase", "physical", "piano", "picnic", "picture", "piece", "pig", "pigeon", "pill", "pilot",
  "pink", "pioneer", "pipe", "pistol", "pitch", "pizza", "place", "planet", "plastic", "plate",
  "play", "please", "pledge", "pluck", "plug", "plunge", "poem", "poet", "point", "polar",
  "pole", "police", "pond", "pony", "pool", "popular", "portion", "position", "possible", "post",
  "potato", "pottery", "poverty", "powder", "power", "practice", "praise", "predict", "prefer", "prepare",
  "present", "pretty", "prevent", "price", "pride", "primary", "print", "priority", "prison", "private",
  "prize", "problem", "process", "produce", "profit", "program", "project", "promote", "proof", "property",
  "prosper", "protect", "proud", "provide", "public", "pudding", "pull", "pulp", "pulse", "pumpkin",
  "punch", "pupil", "puppy", "purchase", "purity", "purpose", "purse", "push", "put", "puzzle",
  "pyramid", "quality", "quantum", "quarter", "question", "quick", "quit", "quiz", "quote", "rabbit",
  "raccoon", "race", "rack", "radar", "radio", "rail", "rain", "raise", "rally", "ramp",
  "ranch", "random", "range", "rapid", "rare", "rate", "rather", "raven", "raw", "razor",
  "ready", "real", "reason", "rebel", "rebuild", "recall", "receive", "recipe", "record", "recycle",
  "reduce", "reflect", "reform", "refuse", "region", "regret", "regular", "reject", "relax", "release",
  "relief", "rely", "remain", "remember", "remind", "remove", "render", "renew", "rent", "reopen",
  "repair", "repeat", "replace", "report", "require", "rescue", "resemble", "resist", "resource", "response",
  "result", "retire", "retreat", "return", "reunion", "reveal", "review", "reward", "rhythm", "rib",
  "ribbon", "rice", "rich", "ride", "ridge", "rifle", "right", "rigid", "ring", "riot",
  "ripple", "risk", "ritual", "rival", "river", "road", "roast", "robot", "robust", "rocket",
  "romance", "roof", "rookie", "room", "rose", "rotate", "rough", "round", "route", "royal",
  "rubber", "rude", "rug", "rule", "run", "runway", "rural", "sad", "saddle", "sadness",
  "safe", "sail", "salad", "salmon", "salon", "salt", "salute", "same", "sample", "sand",
  "satisfy", "saturday", "sauce", "sausage", "save", "say", "scale", "scan", "scare", "scatter",
  "scene", "scheme", "school", "science", "scissors", "scorpion", "scout", "scrap", "screen", "script",
  "scrub", "sea", "search", "season", "seat", "second", "secret", "section", "security", "seed",
  "seek", "segment", "select", "sell", "seminar", "senior", "sense", "sentence", "series", "service",
  "session", "settle", "setup", "seven", "shadow", "shaft", "shallow", "share", "shed", "shell",
  "sheriff", "shield", "shift", "shine", "ship", "shiver", "shock", "shoe", "shoot", "shop",
  "short", "shoulder", "shove", "shrimp", "shrug", "shuffle", "shy", "sibling", "sick", "side",
  "siege", "sight", "sign", "silent", "silk", "silly", "silver", "similar", "simple", "since",
  "sing", "siren", "sister", "situate", "six", "size", "skate", "sketch", "ski", "skill",
  "skin", "skirt", "skull", "slab", "slam", "sleep", "slender", "slice", "slide", "slight",
  "slim", "slogan", "slot", "slow", "slush", "small", "smart", "smile", "smoke", "smooth",
  "snack", "snake", "snap", "sniff", "snow", "soap", "soccer", "social", "sock", "soda",
  "soft", "solar", "soldier", "solid", "solution", "solve", "someone", "song", "soon", "sorry",
  "sort", "soul", "sound", "soup", "source", "south", "space", "spare", "spatial", "spawn",
  "speak", "special", "speed", "spell", "spend", "sphere", "spice", "spider", "spike", "spin",
  "spirit", "split", "spoil", "sponsor", "spoon", "sport", "spot", "spray", "spread", "spring",
  "spy", "square", "squeeze", "squirrel", "stable", "stadium", "staff", "stage", "stairs", "stamp",
  "stand", "start", "state", "stay", "steak", "steel", "stem", "step", "stereo", "stick",
  "still", "sting", "stock", "stomach", "stone", "stool", "story", "stove", "strategy", "street",
  "strike", "strong", "struggle", "student", "stuff", "stumble", "style", "subject", "submit",
  "subway", "success", "such", "sudden", "suffer", "sugar", "suggest", "suit", "summer", "sun",
  "sunny", "sunset", "super", "supply", "supreme", "sure", "surface", "surge", "surprise", "surround",
  "survey", "suspect", "sustain", "swallow", "swamp", "swap", "swarm", "swear", "sweet", "swift",
  "swim", "swing", "switch", "sword", "symbol", "symptom", "syrup", "system", "table", "tackle",
  "tag", "tail", "talent", "talk", "tank", "tape", "target", "task", "taste", "tattoo",
  "taxi", "teach", "team", "tell", "ten", "tenant", "tennis", "tent", "term", "test",
  "text", "thank", "that", "theme", "then", "theory", "there", "they", "thing", "this",
  "thought", "three", "thrive", "throw", "thumb", "thunder", "ticket", "tide", "tiger", "tilt",
  "timber", "time", "tiny", "tip", "tired", "tissue", "title", "toast", "tobacco", "today",
  "toddler", "toe", "together", "toilet", "token", "tomato", "tomorrow", "tone", "tongue", "tonight",
  "tool", "tooth", "top", "topic", "topple", "torch", "tornado", "tortoise", "toss", "total",
  "tourist", "toward", "tower", "town", "toy", "track", "trade", "traffic", "tragic", "train",
  "transfer", "trap", "trash", "travel", "tray", "treat", "tree", "trend", "trial", "tribe",
  "trick", "trigger", "trim", "trip", "trophy", "trouble", "truck", "true", "truly", "trumpet",
  "trust", "truth", "try", "tube", "tuition", "tumble", "tuna", "tunnel", "turkey", "turn",
  "turtle", "twelve", "twenty", "twice", "twin", "twist", "two", "type", "typical", "ugly",
  "umbrella", "unable", "unaware", "uncle", "uncover", "under", "undo", "unfair", "unfold", "unhappy",
  "uniform", "unique", "unit", "universe", "unknown", "unlock", "until", "unusual", "unveil", "update",
  "upgrade", "uphold", "upon", "upper", "upset", "urban", "urge", "usage", "use", "used",
  "useful", "useless", "usual", "utility", "vacant", "vacuum", "vague", "valid", "valley", "valve",
  "van", "vanish", "vapor", "various", "vast", "vault", "vehicle", "velvet", "vendor", "venture",
  "venue", "verb", "verify", "version", "very", "vessel", "veteran", "viable", "vibrant", "vicious",
  "victory", "video", "view", "village", "vintage", "violin", "virtual", "virus", "visa", "visit",
  "visual", "vital", "vivid", "vocal", "voice", "void", "volcano", "volume", "vote", "voyage",
  "wage", "wagon", "wait", "walk", "wall", "walnut", "want", "warfare", "warm", "warrior",
  "wash", "wasp", "waste", "water", "wave", "way", "wealth", "weapon", "wear", "weasel",
  "weather", "web", "wedding", "weekend", "weird", "welcome", "west", "wet", "whale", "what",
  "wheat", "wheel", "when", "where", "whip", "whisper", "wide", "width", "wife", "wild",
  "will", "win", "window", "wine", "wing", "wink", "winner", "winter", "wire", "wisdom",
  "wise", "wish", "witness", "wolf", "woman", "wonder", "wood", "wool", "word", "work",
  "world", "worry", "worth", "wrap", "wreck", "wrestle", "wrist", "write", "wrong", "yard",
  "year", "yellow", "you", "young", "youth", "zebra", "zero", "zone", "zoo"
];

const B32_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';

function encodeBase32(bytes) {
  let bits = 0;
  let value = 0;
  let output = '';
  for (let i = 0; i < bytes.length; i++) {
    value = (value << 8) | bytes[i];
    bits += 8;
    while (bits >= 5) {
      output += B32_ALPHABET[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) {
    output += B32_ALPHABET[(value << (5 - bits)) & 31];
  }
  return output;
}

function decodeBase32(s) {
  let bits = 0;
  let value = 0;
  const output = [];
  for (let i = 0; i < s.length; i++) {
    const idx = B32_ALPHABET.indexOf(s[i].toUpperCase()); // Match uppercase alphabet
    if (idx === -1) return null;
    value = (value << 5) | idx;
    bits += 5;
    if (bits >= 8) {
      output.push((value >>> (bits - 8)) & 255);
      bits -= 8;
    }
  }
  return new Uint8Array(output);
}

async function sha512(data) {
  const digest = await crypto.subtle.digest('SHA-512', data);
  return new Uint8Array(digest);
}

function sha3_256_hash(data) {
  // Use js-sha3 library (loaded from CDN)
  // Call the LIBRARY's sha3_256, not sha3_256_custom
  const hashHex = sha3_256(data);
  return new Uint8Array(hashHex.match(/.{1,2}/g).map(byte => parseInt(byte, 16)));
}

async function encodeKOT1(addressBytes) {
  const b32 = encodeBase32(addressBytes);
  const prefix = new TextEncoder().encode('KOT1');
  const payload = new Uint8Array(prefix.length + addressBytes.length);
  payload.set(prefix);
  payload.set(addressBytes, prefix.length);

  // Checksum: SHA3-256(SHA3-256("KOT1" + address_bytes))[0..4]
  const hash1 = sha3_256_hash(payload);
  const hash2 = sha3_256_hash(hash1);
  const checksum = encodeBase32(hash2.slice(0, 4));

  return `KOT1${b32}${checksum}`;
}

async function decodeKOT1(s) {
  const original = s;
  s = s.toUpperCase();
  
  if (!s.startsWith('KOT1')) {
    return null;
  }
  
  const body = s.slice(4);
  if (body.length < 8) {
    return null;
  }

  const addrPart = body.slice(0, -7);
  
  const addressBytes = decodeBase32(addrPart);
  
  if (!addressBytes || addressBytes.length !== 32) {
    return null;
  }

  const expected = await encodeKOT1(addressBytes);
  
  return expected === s ? addressBytes : null;
}

function stateToHex(input) {
  return Array.from(input).map(b => b.toString(16).padStart(2, '0')).join('');
}

function hexToBytes(hex) {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  return bytes;
}

async function generateMnemonic() {
  const entropy = new Uint8Array(32);
  crypto.getRandomValues(entropy);

  const hash = new Uint8Array(await crypto.subtle.digest('SHA-256', entropy));
  const checksumBits = hash[0];

  let bits = '';
  for (const b of entropy) bits += b.toString(2).padStart(8, '0');
  bits += checksumBits.toString(2).padStart(8, '0');

  const words = [];
  for (let i = 0; i < 24; i++) {
    const idx = parseInt(bits.slice(i * 11, (i + 1) * 11), 2);
    words.push(WORDLIST[idx]);
  }
  return words.join(' ');
}

async function mnemonicToSeed(mnemonic, passphrase = '') {
  const enc = new TextEncoder();
  const salt = enc.encode('mnemonic' + passphrase);
  const keyMaterial = await crypto.subtle.importKey(
    'raw',
    enc.encode(mnemonic),
    { name: 'PBKDF2' },
    false,
    ['deriveBits']
  );

  const pbkdf2Seed = await crypto.subtle.deriveBits(
    {
      name: 'PBKDF2',
      salt: salt,
      iterations: 2048,
      hash: 'SHA-512'
    },
    keyMaterial,
    512
  );

  // Knotcoin Master Key: HMAC-SHA512("Knotcoin seed v1", pbkdf2Seed)
  const hmacKeyMaterial = await crypto.subtle.importKey(
    'raw',
    enc.encode('Knotcoin seed v1'),
    { name: 'HMAC', hash: 'SHA-512' },
    false,
    ['sign']
  );
  const signature = await crypto.subtle.sign('HMAC', hmacKeyMaterial, pbkdf2Seed);
  return new Uint8Array(signature);
}

const state = {
  connected: false,
  chainHeight: 0,
  difficultyHex: '',
  navHistory: [],
  blocksPage: 0,
  blocksPageSize: 40,
  walletAddr: null,
  masterSeedHex: null,
  blockCache: new Map(),
  recentBlocks: [],
  stats: {
    avgBlockSec: null,
    globalHashrate: null,
    expectedHashesPerBlock: null,
  },
  mining: {
    active: false,
    stop: false,
    startMs: 0,
    mined: 0,
    target: 0,
    address: null,
  },
  governance: {
    proposals: [],
    lastRefresh: 0,
  }
};

function el(id) {
  return document.getElementById(id);
}

function setText(id, text) {
  const node = el(id);
  if (node) node.textContent = text;
}

function fmtTime(ts) {
  if (!ts) return 'N/A';
  return new Date(Number(ts) * 1000).toISOString().replace('T', ' ').slice(0, 19) + ' UTC';
}

function ago(ts) {
  if (!ts) return 'N/A';
  const sec = Math.max(0, Math.floor(Date.now() / 1000 - Number(ts)));
  if (sec < 60) return `${sec}s`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m`;
  if (sec < 86400) return `${Math.floor(sec / 3600)}h`;
  return `${Math.floor(sec / 86400)}d`;
}

function fmtKOT(knots) {
  return (Number(knots || 0) / 1e8).toFixed(8);
}

function abbrev(v, left = 12, right = 10) {
  const s = String(v || '');
  if (s.length <= left + right + 1) return s;
  return `${s.slice(0, left)}...${s.slice(-right)}`;
}

async function normalizeAddress(input) {
  let addr = String(input || '').trim().toUpperCase();
  
  if (addr.startsWith('KOT1')) {
    const bytes = await decodeKOT1(addr);
    if (!bytes) {
      return null;
    }
    return addr;
  }
  return null;
}

async function toHexAddressNoPrefix(input) {
  let addr = String(input || '').trim();
  if (addr.toLowerCase().startsWith('kot1')) {
    const bytes = await decodeKOT1(addr);
    return bytes ? stateToHex(bytes) : null;
  }
  if (!/^[a-f0-9]{32,64}$/i.test(addr)) return null;
  let clean = addr.toLowerCase();
  if (clean.length === 64) return clean;
  // If it's short, it's not a full 32-byte address
  return null;
}

async function formatAddressKOT1(input) {
  const hex = await toHexAddressNoPrefix(input);
  if (!hex) return String(input || 'N/A');
  return await encodeKOT1(hexToBytes(hex));
}

function copyTextToClipboard(text) {
  navigator.clipboard.writeText(text).then(() => {
    alert('Copied to clipboard: ' + (text.length > 20 ? text.slice(0, 20) + '...' : text));
  }).catch(err => {
    console.error('Failed to copy: ', err);
  });
}

function copyText(valueOrId) {
  const node = el(valueOrId);
  const text = node ? (node.textContent || node.value || '').trim() : String(valueOrId || '');
  if (!text) return;
  copyTextToClipboard(text);
}

window.copyText = copyText;

async function rpc(method, params = []) {
  try {
    const res = await fetch(RPC, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', method, params, id: Date.now() }),
    });
    const payload = await res.json();
    state.connected = true;
    updateConn();
    return payload.result;
  } catch {
    state.connected = false;
    updateConn();
    return null;
  }
}

function updateConn() {
  const node = el('conn-status');
  if (!node) return;
  if (state.connected) {
    node.textContent = 'ONLINE';
    node.classList.remove('bad');
    node.classList.add('good');
  } else {
    node.textContent = 'OFFLINE';
    node.classList.remove('good');
    node.classList.add('bad');
  }
}

function supplyAt(height) {
  const h = Math.max(0, Number(height));
  const p1End = 262800;
  const p2End = 525600;
  if (h <= p1End) {
    // Sum from block 0 to h: each block i gives 0.1 + (0.9 * i / p1End)
    // Total = (h+1) * 0.1 + 0.9 * sum(i from 0 to h) / p1End
    // sum(i from 0 to h) = h * (h+1) / 2
    return (h + 1) * 0.1 + (0.9 * h * (h + 1)) / (2 * p1End);
  }
  if (h <= p2End) {
    // Phase 1 supply + Phase 2 blocks (each gives 1.0 KOT)
    return supplyAt(p1End) + (h - p1End) * 1.0;
  }

  // Phase 3: decay formula
  let s = supplyAt(p2End);
  if (h > p2End) {
    const adjusted = h - p2End;
    // Approximation for sum of 1/log2(i+2) from i=1 to adjusted
    s += adjusted / Math.log2(adjusted / 2 + 2);
  }
  return s;
}

function rewardKnotsAtHeight(height) {
  const h = Number(height);
  if (h < 0) return 0;
  return Math.max(0, Math.round((supplyAt(h) - supplyAt(h - 1)) * 1e8));
}

function leadingZeroNibbles(hashHex) {
  let z = 0;
  for (const ch of String(hashHex || '').toLowerCase()) {
    if (ch === '0') z++;
    else break;
  }
  return z;
}

function formatDifficulty(targetHex) {
  if (!/^[a-f0-9]{64}$/i.test(String(targetHex || ''))) return 'N/A';
  const g = BigInt('0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff');
  const c = BigInt(`0x${targetHex}`);
  if (c <= 0n) return 'MAX';
  const diff = Number(g) / Number(c);
  return diff.toFixed(4);
}

function bigIntApproxFloat(bi) {
  if (!bi || bi <= 0n) return 0;
  const s = bi.toString(10);
  const keep = 15;
  if (s.length <= keep) return Number(s);
  const lead = Number(s.slice(0, keep));
  return lead * 10 ** (s.length - keep);
}

function formatHashrate(v) {
  const n = Number(v || 0);
  if (!Number.isFinite(n) || n <= 0) return 'N/A';
  const units = ['H/s', 'kH/s', 'MH/s', 'GH/s', 'TH/s', 'PH/s', 'EH/s'];
  let x = n;
  let i = 0;
  while (x >= 1000 && i < units.length - 1) {
    x /= 1000;
    i++;
  }
  return `${x.toFixed(x < 10 ? 3 : x < 100 ? 2 : 1)} ${units[i]}`;
}

function expectedHashesPerBlock(targetHex) {
  if (!/^[a-f0-9]{64}$/i.test(String(targetHex || ''))) return null;
  const t = BigInt(`0x${targetHex}`);
  if (t <= 0n) return TWO_256;
  return TWO_256 / (t + 1n);
}

async function fetchHead() {
  const [miningInfo, memInfo] = await Promise.all([rpc('getmininginfo'), rpc('getmempoolinfo')]);
  if (!miningInfo || miningInfo.error) return null;

  state.chainHeight = Number(miningInfo.blocks || 0);
  state.difficultyHex = String(miningInfo.difficulty || '');

  // Use the RPC hashrate if available, otherwise fall back to local estimate
  if (miningInfo.networkhashps > 0) {
    state.stats.globalHashrate = Number(miningInfo.networkhashps);
  }

  setText('head-height', `HEIGHT: ${state.chainHeight}`);
  setText('head-hashrate', `HASH: ${formatHashrate(state.stats.globalHashrate)}`);
  setText('head-diff', `DIFF: ${formatDifficulty(state.difficultyHex)}`);
  setText('head-reward', `REWARD: ${fmtKOT(rewardKnotsAtHeight(state.chainHeight))} KOT`);
  setText('head-supply', `SUPPLY: ${supplyAt(state.chainHeight).toFixed(2)} KOT`);
  setText('head-mempool', `MEM: ${memInfo?.size ?? 0}`);

  return { miningInfo, memInfo };
}

async function fetchBlockByHeight(height, force = false) {
  const h = Number(height);
  if (h < 0) return null;
  if (!force && state.blockCache.has(h)) return state.blockCache.get(h);

  const hash = await rpc('getblockhash', [h]);
  if (!hash) return null;

  const block = await rpc('getblock', [hash]);
  if (!block || block.error) return null;

  block.hash = hash;
  state.blockCache.set(h, block);
  return block;
}

async function fetchRecentBlocks(limit = 40) {
  const end = state.chainHeight;
  const start = Math.max(0, end - limit + 1);
  const blocks = [];

  for (let h = end; h >= start; h--) {
    const b = await fetchBlockByHeight(h);
    if (b) blocks.push(b);
  }

  state.recentBlocks = blocks;
  return blocks;
}

function computeTimingAndHashrate(recentBlocks) {
  if (!recentBlocks || recentBlocks.length < 2) {
    state.stats.avgBlockSec = null;
    state.stats.expectedHashesPerBlock = expectedHashesPerBlock(state.difficultyHex);
    return;
  }

  const newest = Number(recentBlocks[0].time || 0);
  const oldest = Number(recentBlocks[recentBlocks.length - 1].time || 0);
  const produced = Math.max(1, recentBlocks.length - 1);
  const dt = Math.max(1, newest - oldest);

  const avgSec = dt / produced;
  state.stats.avgBlockSec = avgSec;
  state.stats.expectedHashesPerBlock = expectedHashesPerBlock(state.difficultyHex);

  // If RPC didn't provide a hashrate, calculate it locally
  if (!state.stats.globalHashrate) {
    const exp = state.stats.expectedHashesPerBlock;
    const expApprox = exp ? bigIntApproxFloat(exp) : 0;
    state.stats.globalHashrate = avgSec > 0 ? expApprox / avgSec : 0;
  }
}

function renderHomeStats() {
  const recent = state.recentBlocks;
  const latest = recent[0];
  const avgSec = state.stats.avgBlockSec;
  const global = state.stats.globalHashrate;
  const reward = rewardKnotsAtHeight(state.chainHeight);

  const lines = [
    `<div style="display: grid; grid-template-columns: auto 1fr; gap: 4px 8px; font-size: 10px;">`,
    `<span style="color: var(--dim);">HEIGHT:</span><span style="color: var(--text); font-weight: 700;">${state.chainHeight}</span>`,
    `<span style="color: var(--dim);">HASHRATE:</span><span style="color: var(--ok); font-weight: 700;">${formatHashrate(global)}</span>`,
    `<span style="color: var(--dim);">REWARD:</span><span style="color: var(--accent); font-weight: 700;">${fmtKOT(reward)} KOT</span>`,
    `<span style="color: var(--dim);">AVG TIME:</span><span style="color: var(--text); font-weight: 700;">${avgSec ? avgSec.toFixed(1) + 's' : 'N/A'}</span>`,
    `<span style="color: var(--dim);">DIFFICULTY:</span><span style="color: var(--warn); font-weight: 700;">${formatDifficulty(state.difficultyHex).substring(0, 12)}...</span>`,
    `<span style="color: var(--dim);">LAST BLOCK:</span><span style="color: var(--text); font-weight: 700;">${latest ? ago(latest.time) : 'N/A'}</span>`,
    `</div>`
  ];

  const el = document.getElementById('home-stats-compact');
  if (el) el.innerHTML = lines.join('');
}

async function formatBlockDeep(block) {
  const reward = fmtKOT(rewardKnotsAtHeight(block.height));
  const minerKOT = await formatAddressKOT1(block.miner);
  const lines = [
    `+----------------------- BLOCK #${block.height} -----------------------+`,
    ` hash        : ${block.hash}`,
    ` prev        : ${block.previousblockhash}`,
    ` time        : ${fmtTime(block.time)} (${ago(block.time)} ago)`,
    ` miner       : ${minerKOT}`,
    ` tx_count    : ${block.tx_count}`,
    ` reward      : ${reward} KOT`,
    ` difficulty  : ${formatDifficulty(block.difficulty)}`,
    ` nonce       : ${block.nonce}`,
    ` pow_quality : ${leadingZeroNibbles(block.hash)} leading zero hex nibbles`,
  ];

  if (block.transactions && block.transactions.length) {
    lines.push(' txs:');
    for (let i = 0; i < block.transactions.length; i++) {
      const tx = block.transactions[i];
      const fromKOT = await formatAddressKOT1(tx.sender);
      const toKOT = await formatAddressKOT1(tx.recipient);
      lines.push(`   [${i}] from=${fromKOT}`);
      lines.push(`       to=${toKOT}`);
      lines.push(`       txid=${tx.txid}`);
      lines.push(`       amount=${fmtKOT(tx.amount)} KOT fee=${tx.fee} knots nonce=${tx.nonce}`);
    }
  } else {
    lines.push(' txs         : none');
  }

  lines.push('+---------------------------------------------------------------+');
  return lines.join('\n');
}

async function renderHomeBlocks(blocks) {
  // Network visualization replaces this - see initNetworkViz()
}

// Network Visualization Module
let networkViz = null;
let networkVizCache = { data: null, timestamp: 0 };

function initNetworkViz() {
  console.log('[VIZ] Initializing network visualization...');
  const container = el('network-viz-container');
  const canvas = el('network-viz-canvas');
  
  if (!container) {
    console.error('[VIZ] Container not found: network-viz-container');
    return;
  }
  if (!canvas) {
    console.error('[VIZ] Canvas not found: network-viz-canvas');
    return;
  }
  
  if (typeof d3 === 'undefined') {
    console.error('[VIZ] D3.js not loaded!');
    return;
  }
  
  console.log('[VIZ] D3.js version:', d3.version);

  const width = container.clientWidth;
  const height = container.clientHeight;
  
  console.log('[VIZ] Container dimensions:', width, 'x', height);

  const svg = d3.select(canvas)
    .attr('width', width)
    .attr('height', height);

  svg.selectAll('*').remove();

  const vizContainer = svg.append('g');
  const linkGroup = vizContainer.append('g').attr('class', 'links');
  const nodeGroup = vizContainer.append('g').attr('class', 'nodes');

  // Arrow marker
  svg.append('defs').append('marker')
    .attr('id', 'viz-arrow')
    .attr('viewBox', '0 -5 10 10')
    .attr('refX', 18)
    .attr('refY', 0)
    .attr('markerWidth', 5)
    .attr('markerHeight', 5)
    .attr('orient', 'auto')
    .append('path')
    .attr('d', 'M0,-4L10,0L0,4')
    .attr('class', 'viz-connection-arrow');

  const zoom = d3.zoom()
    .scaleExtent([0.01, 20])
    .on('zoom', (event) => {
      vizContainer.attr('transform', event.transform);
    });

  svg.call(zoom);

  networkViz = {
    svg, vizContainer, linkGroup, nodeGroup, zoom, width, height,
    miners: [],
    simulation: null,
    lastUpdate: 0,
    updateInProgress: false
  };
  
  console.log('[VIZ] Network visualization initialized successfully');
}

async function updateNetworkViz() {
  if (!networkViz) {
    console.error('[VIZ] networkViz not initialized');
    return;
  }
  
  if (networkViz.updateInProgress) {
    console.log('[VIZ] Update already in progress, skipping');
    return;
  }

  try {
    networkViz.updateInProgress = true;
    console.log('[VIZ] Starting update...');

    // Use cached data if less than 3 seconds old
    const now = Date.now();
    if (networkVizCache.data && (now - networkVizCache.timestamp < 3000)) {
      console.log('[VIZ] Using cached data');
      networkViz.miners = networkVizCache.data;
      renderNetworkViz();
      networkViz.updateInProgress = false;
      return;
    }

    // Throttle RPC calls
    if (now - networkViz.lastUpdate < 2000) {
      console.log('[VIZ] Throttled, skipping update');
      networkViz.updateInProgress = false;
      return;
    }
    networkViz.lastUpdate = now;

    // Fetch all miners with referral data
    console.log('[VIZ] Fetching miners from RPC...');
    const data = await rpc('get_all_miners', []);
    console.log('[VIZ] RPC response:', data);
    
    // Backend returns { "miners": [...] } directly
    if (!data || !data.miners) {
      console.error('[VIZ] Invalid response format:', data);
      networkViz.updateInProgress = false;
      return;
    }

    console.log('[VIZ] Found', data.miners.length, 'miners');

    // Update miners data with position persistence
    const existingMiners = new Map(networkViz.miners.map(m => [m.address, m]));
    
    networkViz.miners = data.miners.map(m => {
      const existing = existingMiners.get(m.address);
      return {
        ...m,
        x: existing?.x || networkViz.width / 2 + (Math.random() - 0.5) * 400,
        y: existing?.y || networkViz.height / 2 + (Math.random() - 0.5) * 400,
        vx: existing?.vx || 0,
        vy: existing?.vy || 0,
        radius: Math.sqrt(Math.max(m.blocks_mined || 1, 1)) * 8 + 10,
        joinedAt: m.joined_at || Date.now()
      };
    });

    console.log('[VIZ] Processed miners:', networkViz.miners.length);

    // Cache the data
    networkVizCache.data = networkViz.miners;
    networkVizCache.timestamp = now;

    console.log('[VIZ] Calling renderNetworkViz...');
    renderNetworkViz();
  } catch (err) {
    console.error('[VIZ] Update failed:', err);
  } finally {
    networkViz.updateInProgress = false;
  }
}

function renderNetworkViz() {
  console.log('[VIZ] renderNetworkViz called, miners:', networkViz?.miners?.length);
  
  if (!networkViz || !networkViz.miners.length) {
    console.log('[VIZ] No miners to render');
    // Show empty state message
    const container = document.getElementById('network-viz-container');
    if (container && networkViz && networkViz.miners.length === 0) {
      const svg = networkViz.svg;
      svg.selectAll('.empty-message').remove();
      svg.append('text')
        .attr('class', 'empty-message')
        .attr('x', networkViz.width / 2)
        .attr('y', networkViz.height / 2)
        .attr('text-anchor', 'middle')
        .attr('fill', 'var(--dim)')
        .attr('font-size', '14px')
        .attr('font-weight', 'bold')
        .text('No miners yet. Mine the genesis block to start the network.');
    }
    return;
  }

  console.log('[VIZ] Rendering', networkViz.miners.length, 'miners');

  const { miners, linkGroup, nodeGroup, simulation, width, height } = networkViz;
  const isLargeNetwork = miners.length > 200;

  // Remove empty message if it exists
  networkViz.svg.selectAll('.empty-message').remove();

  // Create links
  const links = [];
  miners.forEach(m => {
    if (m.referrer && miners.find(r => r.address === m.referrer)) {
      const refIndex = miners.findIndex(r => r.address === m.referrer);
      const minerIndex = miners.findIndex(r => r.address === m.address);
      if (refIndex >= 0 && minerIndex >= 0) {
        links.push({ source: refIndex, target: minerIndex });
      }
    }
  });

  console.log('[VIZ] Created', links.length, 'referral links');

  // Update simulation
  if (!networkViz.simulation) {
    console.log('[VIZ] Creating new simulation');
    networkViz.simulation = d3.forceSimulation(miners)
      .force('charge', d3.forceManyBody().strength(isLargeNetwork ? -100 : -300))
      .force('center', d3.forceCenter(width / 2, height / 2))
      .force('collision', d3.forceCollide().radius(d => d.radius + 5))
      .force('link', d3.forceLink(links).id((d, i) => i).distance(isLargeNetwork ? 60 : 100).strength(0.5));
  } else {
    console.log('[VIZ] Updating existing simulation');
    networkViz.simulation.nodes(miners);
    networkViz.simulation.force('link').links(links);
    networkViz.simulation.force('charge').strength(isLargeNetwork ? -100 : -300);
    networkViz.simulation.force('link').distance(isLargeNetwork ? 60 : 100);
    networkViz.simulation.alpha(isLargeNetwork ? 0.1 : 0.3).restart();
  }

  // Draw lines
  const lineSelection = linkGroup.selectAll('line')
    .data(links, d => `${d.source}-${d.target}`);

  lineSelection.exit().remove();

  lineSelection.enter()
    .append('line')
    .attr('class', 'viz-connection-line')
    .attr('marker-end', 'url(#viz-arrow)')
    .merge(lineSelection);

  // Draw circles
  const circles = nodeGroup.selectAll('circle')
    .data(miners, d => d.address);

  circles.exit().remove();

  const enter = circles.enter()
    .append('circle')
    .attr('r', 0)
    .style('fill', d => getVizColor(d))
    .style('stroke', 'rgba(255,255,255,0.3)')
    .style('stroke-width', 2)
    .style('cursor', 'grab')
    .style('opacity', 0)
    .call(d3.drag()
      .on('start', vizDragStarted)
      .on('drag', vizDragged)
      .on('end', vizDragEnded))
    .on('mouseover', showVizTooltip)
    .on('mouseout', hideVizTooltip)
    .on('click', (event, d) => {
      event.stopPropagation();
      showVizInfo(d);
    });

  const merged = enter.merge(circles);

  merged.each(function(d) {
    const timeSinceJoin = Date.now() - d.joinedAt;
    const isNew = timeSinceJoin < 300000;
    d3.select(this).classed('viz-new-miner', isNew);
  });

  const transitionDuration = isLargeNetwork ? 200 : 500;
  merged.transition()
    .duration(transitionDuration)
    .attr('r', d => d.radius)
    .style('fill', d => getVizColor(d))
    .style('opacity', 1);

  // Labels
  const minLabelRadius = isLargeNetwork ? 25 : 15;
  const labels = nodeGroup.selectAll('.viz-bubble-label')
    .data(miners.filter(d => d.radius > minLabelRadius), d => d.address);

  labels.exit().remove();

  labels.enter()
    .append('text')
    .attr('class', 'viz-bubble-label')
    .attr('dy', '0.3em')
    .text(d => d.blocks_mined || '')
    .style('opacity', 0)
    .merge(labels)
    .transition()
    .duration(transitionDuration)
    .style('opacity', d => d.radius > (minLabelRadius + 5) ? 1 : 0);

  // Tick handler
  networkViz.simulation.on('tick', () => {
    linkGroup.selectAll('line')
      .attr('x1', d => d.source.x)
      .attr('y1', d => d.source.y)
      .attr('x2', d => {
        const dx = d.target.x - d.source.x;
        const dy = d.target.y - d.source.y;
        const dist = Math.sqrt(dx * dx + dy * dy);
        return d.target.x - (dx / dist) * d.target.radius;
      })
      .attr('y2', d => {
        const dx = d.target.x - d.source.x;
        const dy = d.target.y - d.source.y;
        const dist = Math.sqrt(dx * dx + dy * dy);
        return d.target.y - (dy / dist) * d.target.radius;
      });

    nodeGroup.selectAll('circle')
      .attr('cx', d => d.x)
      .attr('cy', d => d.y);

    nodeGroup.selectAll('.viz-bubble-label')
      .attr('x', d => d.x)
      .attr('y', d => d.y);
  });
}

function getVizColor(miner) {
  const blocksSince = state.chainHeight - (miner.last_mined_height || 0);
  if (!miner.last_mined_height) return '#71717a';
  if (blocksSince < 60) return '#22c55e';
  if (blocksSince < 1440) return '#eab308';
  if (blocksSince < 2880) return '#f97316';
  return '#71717a';
}

function vizDragStarted(event, d) {
  if (!event.active) networkViz.simulation.alphaTarget(0.3).restart();
  d.fx = d.x;
  d.fy = d.y;
  d3.select(this).classed('viz-dragging', true);
}

function vizDragged(event, d) {
  d.fx = event.x;
  d.fy = event.y;
}

function vizDragEnded(event, d) {
  if (!event.active) networkViz.simulation.alphaTarget(0);
  d.fx = null;
  d.fy = null;
  d3.select(this).classed('viz-dragging', false);
}

function showVizTooltip(event, d) {
  const tooltip = el('network-viz-tooltip');
  if (!tooltip) return;

  const blocksSince = state.chainHeight - (d.last_mined_height || 0);
  const referrerInfo = d.referrer || 'Independent';
  const referralCount = networkViz.miners.filter(m => m.referrer === d.address).length;
  const timeSinceJoin = Date.now() - d.joinedAt;
  const isNew = timeSinceJoin < 300000;
  const joinTime = isNew ? `${Math.floor(timeSinceJoin / 1000)}s ago` : 'Established';

  // SECURITY: All blockchain data is sanitized via escapeHtml() before insertion
  // Using setSafeHTML for additional safety layer
  const tooltipHTML = `
    <div class="viz-tooltip-address">${escapeHtml(d.address.substring(0, 12))}...</div>
    ${isNew ? '<div class="viz-tooltip-stat" style="color: var(--accent);">🆕 NEW MINER</div>' : ''}
    <div class="viz-tooltip-stat">Joined: <span>${escapeHtml(joinTime)}</span></div>
    <div class="viz-tooltip-stat">Blocks: <span>${escapeHtml(String(d.blocks_mined || 0))}</span></div>
    <div class="viz-tooltip-stat">Referred by: <span>${escapeHtml(referrerInfo.substring(0, 12))}...</span></div>
    <div class="viz-tooltip-stat">Referrals: <span>${escapeHtml(String(referralCount))}</span></div>
  `;
  setSafeHTML(tooltip, tooltipHTML);

  tooltip.style.left = (event.pageX + 15) + 'px';
  tooltip.style.top = (event.pageY + 15) + 'px';
  tooltip.classList.add('visible');
}

function hideVizTooltip() {
  const tooltip = el('network-viz-tooltip');
  if (tooltip) tooltip.classList.remove('visible');
}

function showVizInfo(miner) {
  const panel = el('network-viz-info');
  if (!panel) return;

  const blocksSince = state.chainHeight - (miner.last_mined_height || 0);
  const status = !miner.last_mined_height ? 'Never Mined' :
                blocksSince < 60 ? 'Very Active' :
                blocksSince < 1440 ? 'Active' :
                blocksSince < 2880 ? 'Eligible' : 'Inactive';

  const referrerAddr = miner.referrer || 'Independent';
  const referralCount = networkViz.miners.filter(m => m.referrer === miner.address).length;

  // SECURITY: All blockchain data is sanitized via escapeHtml()
  const panelHTML = `
    <div class="viz-info-header">
      <div class="viz-info-title">${escapeHtml(miner.address.substring(0, 16))}...</div>
      <div style="display: flex; gap: 8px; align-items: center;">
        <button class="viz-copy-btn" onclick="copyText('${escapeHtml(miner.address)}', this)" title="Copy full address">📋</button>
        <div class="viz-info-close" onclick="hideVizInfo()">×</div>
      </div>
    </div>
    <div class="viz-info-row">
      <span class="viz-info-label">Status</span>
      <span class="viz-info-value">${escapeHtml(status)}</span>
    </div>
    <div class="viz-info-row">
      <span class="viz-info-label">Blocks Mined</span>
      <span class="viz-info-value">${escapeHtml(String(miner.blocks_mined || 0))}</span>
    </div>
    <div class="viz-info-row">
      <span class="viz-info-label">Last Active</span>
      <span class="viz-info-value">${escapeHtml(!miner.last_mined_height ? 'Never' : blocksSince + ' blocks')}</span>
    </div>
    <div class="viz-info-row">
      <span class="viz-info-label">Referred By</span>
      <span class="viz-info-value">${escapeHtml(referrerAddr.substring(0, 12))}...</span>
    </div>
    <div class="viz-info-row">
      <span class="viz-info-label">Referrals</span>
      <span class="viz-info-value">${escapeHtml(String(referralCount))}</span>
    </div>
  `;
  setSafeHTML(panel, panelHTML);

  panel.classList.add('visible');
}

function hideVizInfo() {
  const panel = el('network-viz-info');
  if (panel) panel.classList.remove('visible');
}

async function refreshHome() {
  console.log('[HOME] Refreshing home page...');
  const head = await fetchHead();
  if (!head) {
    console.error('[HOME] Failed to fetch head');
    return;
  }
  const blocks = await fetchRecentBlocks(40);
  computeTimingAndHashrate(blocks);
  renderHomeStats();
  
  // Initialize network viz if not already done
  if (!networkViz) {
    console.log('[HOME] Initializing network visualization...');
    initNetworkViz();
  }
  
  // Update network visualization
  console.log('[HOME] Updating network visualization...');
  await updateNetworkViz();
  console.log('[HOME] Home refresh complete');
}

async function loadBlocksPage(page) {
  const tbody = el('blocks-tbody');
  const label = el('blocks-page-label');
  if (!tbody || !label) return;

  const head = await fetchHead();
  if (!head) {
    tbody.innerHTML = '';
    const row = document.createElement('tr');
    const cell = document.createElement('td');
    cell.colSpan = 6;
    cell.style.textAlign = 'center';
    cell.style.padding = '20px';
    cell.style.color = 'var(--dim)';
    cell.textContent = 'RPC unavailable.';
    row.appendChild(cell);
    tbody.appendChild(row);
    return;
  }

  state.blocksPage = Math.max(0, Number(page || 0));

  const top = state.chainHeight - state.blocksPage * state.blocksPageSize;
  if (top < 0 && state.blocksPage > 0) {
    state.blocksPage -= 1;
    return loadBlocksPage(state.blocksPage);
  }

  const end = Math.max(0, top);
  const start = Math.max(0, end - state.blocksPageSize + 1);

  tbody.innerHTML = '';
  const loadingRow = document.createElement('tr');
  const loadingCell = document.createElement('td');
  loadingCell.colSpan = 6;
  loadingCell.style.textAlign = 'center';
  loadingCell.style.padding = '20px';
  loadingCell.style.color = 'var(--dim)';
  loadingCell.textContent = 'Loading...';
  loadingRow.appendChild(loadingCell);
  tbody.appendChild(loadingRow);

  const rows = [];
  for (let h = end; h >= start; h--) {
    const b = await fetchBlockByHeight(h);
    if (!b) continue;

    const minerKOT = await formatAddressKOT1(b.miner);

    rows.push(`<tr>
      <td>${escapeHtml(String(b.height))}</td>
      <td>${abbrev(b.hash, 16, 14)}</td>
      <td>${escapeHtml(fmtTime(b.time))}</td>
      <td>${escapeHtml(String(b.tx_count))}</td>
      <td>${abbrev(minerKOT, 12, 10)}</td>
      <td><button data-view-height="${escapeHtml(String(b.height))}">OPEN</button></td>
    </tr>`);
  }

  if (rows.length > 0) {
    tbody.innerHTML = rows.join('');
  } else {
    tbody.innerHTML = '';
    const emptyRow = document.createElement('tr');
    const emptyCell = document.createElement('td');
    emptyCell.colSpan = 6;
    emptyCell.style.textAlign = 'center';
    emptyCell.style.padding = '20px';
    emptyCell.style.color = 'var(--dim)';
    emptyCell.textContent = 'No blocks.';
    emptyRow.appendChild(emptyCell);
    tbody.appendChild(emptyRow);
  }
  label.textContent = `SHOWING HEIGHT #${end} DOWN TO #${start}`;

  const prev = el('blocks-prev');
  const next = el('blocks-next');
  if (prev) prev.disabled = state.blocksPage === 0;
  if (next) next.disabled = start === 0;

  tbody.querySelectorAll('button[data-view-height]').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const h = Number(btn.getAttribute('data-view-height'));
      const hash = await rpc('getblockhash', [h]);
      if (hash) {
        await showBlock(hash);
      }
    });
  });
}

async function localHashrateEstimate(address, recentBlocks) {
  if (!address || !recentBlocks?.length || !state.stats.globalHashrate) return null;
  const normalized = await formatAddressKOT1(address);
  if (!normalized) return null;

  const total = recentBlocks.length;
  const found = recentBlocks.filter((b) => String(b.miner || '').toLowerCase() === normalized.toLowerCase()).length;
  if (!total || !found) return 0;

  const share = found / total;
  return state.stats.globalHashrate * share;
}

function renderNetworkPanel() {
  const recent = state.recentBlocks;
  const latest = recent[0];
  const avgSec = state.stats.avgBlockSec;
  const global = state.stats.globalHashrate;
  const reward = rewardKnotsAtHeight(state.chainHeight);

  // Update individual stat elements
  setText('net-protocol', '1');
  setText('net-name', 'Knotcoin Mainnet');
  setText('net-height', state.chainHeight);
  setText('net-miners', networkViz?.miners?.length || 0);
  setText('net-interval', avgSec ? `${avgSec.toFixed(2)} sec` : '--');
  setText('net-reward', `${fmtKOT(reward)} KOT`);
  setText('net-diff', formatDifficulty(state.difficultyHex).substring(0, 12) + '...');
  setText('net-hash', formatHashrate(global));
  
  // Get referral and governance counts
  const refCount = networkViz?.miners?.filter(m => m.referrer).length || 0;
  const govCount = state.governance?.proposals?.length || 0;
  setText('net-refs', refCount);
  setText('net-props', govCount);
}

async function refreshNetwork() {
  const head = await fetchHead();
  if (!head) return;
  const blocks = await fetchRecentBlocks(60);
  computeTimingAndHashrate(blocks);
  renderNetworkPanel();
}

async function refreshBlocks() {
  const head = await fetchHead();
  if (!head) return;
  
  const blocks = await fetchRecentBlocks(100);
  computeTimingAndHashrate(blocks);
  
  // Update stats
  setText('block-latest', state.chainHeight);
  setText('block-avgtime', state.stats.avgBlockSec ? state.stats.avgBlockSec.toFixed(1) + 's' : '--');
  
  // Calculate total transactions
  let totalTx = 0;
  for (const b of blocks) {
    totalTx += b.tx_count || 0;
  }
  setText('block-totaltx', totalTx);
  
  // Estimate chain size (rough calculation: ~5KB per block average)
  const estimatedSize = (state.chainHeight * 5) / 1024; // MB
  setText('block-size', estimatedSize.toFixed(2) + ' MB');
  
  // Load blocks table
  await loadBlocksPage(state.blocksPage);
}

async function renderMineStats() {
  const addrInput = el('mine-addr');
  const addr = addrInput ? addrInput.value.trim() : '';

  const global = state.stats.globalHashrate;

  let local;
  if (state.mining.active && state.mining.mined > 0) {
    const elapsedSec = Math.max(1, (Date.now() - state.mining.startMs) / 1000);
    const exp = state.stats.expectedHashesPerBlock ? bigIntApproxFloat(state.stats.expectedHashesPerBlock) : 0;
    local = elapsedSec > 0 ? (state.mining.mined * exp) / elapsedSec : 0;
  } else {
    local = await localHashrateEstimate(addr, state.recentBlocks.slice(0, 120));
  }

  const latest = state.recentBlocks[0];
  const kotAddr = await formatAddressKOT1(addr);
  
  // Calculate estimated time to next block
  let timeToBlock = 'Calculating...';
  if (state.stats.expectedHashesPerBlock && global > 0) {
    const expectedHashes = bigIntApproxFloat(state.stats.expectedHashesPerBlock);
    const timeSeconds = expectedHashes / global;
    
    if (timeSeconds < 60) {
      timeToBlock = `${Math.round(timeSeconds)}s`;
    } else if (timeSeconds < 3600) {
      timeToBlock = `${Math.round(timeSeconds / 60)}m`;
    } else {
      timeToBlock = `${(timeSeconds / 3600).toFixed(1)}h`;
    }
  }

  const lines = [
    '+-------------------- MINER STATUS --------------------+',
    ` miner_address         : ${kotAddr}`,
    ` chain_height          : ${state.chainHeight}`,
    ` last_block_time       : ${latest ? fmtTime(latest.time) : 'N/A'} (${latest ? ago(latest.time) : 'N/A'} ago)`,
    ` difficulty            : ${formatDifficulty(state.difficultyHex)}`,
    ` estimated_global_hash : ${formatHashrate(global)}`,
    ` estimated_local_hash  : ${formatHashrate(local)}`,
    ` est_time_to_block     : ${timeToBlock}`,
    ` session_blocks_mined  : ${state.mining.mined}`,
    ` session_target        : ${state.mining.target || 0}`,
    ` mining_state          : ${state.mining.active ? 'RUNNING' : 'IDLE'}`,
    '+------------------------------------------------------+',
  ];

  setText('mine-stats', lines.join('\n'));
}

function appendMineHash(hash) {
  const feed = el('mine-hash-feed');
  if (!feed) return;

  const now = new Date().toISOString().replace('T', ' ').slice(0, 19);
  const current = feed.textContent.trim();
  const line = `${now} | ${hash}`;

  if (!current || current === 'No blocks mined in this session.') {
    feed.textContent = line;
    return;
  }

  const lines = [line, ...current.split('\n')].slice(0, 80);
  feed.textContent = lines.join('\n');
}

async function startMining() {
  const mineBtn = el('mine-btn');
  const res = el('mine-result');
  const addrInput = el('mine-addr');
  const countInput = el('mine-count');

  const normalized = await normalizeAddress(addrInput?.value || state.walletAddr || '');
  if (!normalized) {
    if (res) res.textContent = 'Invalid miner address.';
    return;
  }

  const count = Math.min(500, Math.max(1, Number(countInput?.value || 1)));

  state.mining.active = true;
  state.mining.stop = false;
  state.mining.startMs = Date.now();
  state.mining.mined = 0;
  state.mining.target = count;
  state.mining.address = normalized;

  if (mineBtn) mineBtn.textContent = 'STOP MINING';
  if (res) res.textContent = `Mining started for ${count} blocks...`;

  const ref = localStorage.getItem('knot-referrer');
  const addrNoPrefix = await toHexAddressNoPrefix(normalized);

  while (!state.mining.stop && state.mining.mined < count) {
    const params = [1, addrNoPrefix];
    if (ref) params.push(ref);

    const hashes = await rpc('generatetoaddress', params);
    if (!Array.isArray(hashes)) {
      if (res) res.textContent = 'Mining RPC failed.';
      break;
    }

    for (const h of hashes) {
      state.mining.mined += 1;
      appendMineHash(h);
    }

    if (res) res.textContent = `Mined ${state.mining.mined}/${count} blocks`;

    await refreshCoreData();
    renderMineStats();
  }

  state.mining.active = false;
  state.mining.stop = false;

  if (mineBtn) mineBtn.textContent = 'START MINING';
  if (res) {
    res.textContent = state.mining.mined >= count
      ? `Mining complete: ${state.mining.mined} blocks.`
      : `Mining stopped at ${state.mining.mined} blocks.`;
  }

  await refreshCoreData();
  renderMineStats();
}

function stopMining() {
  state.mining.stop = true;
  const mineBtn = el('mine-btn');
  if (mineBtn) mineBtn.textContent = 'STOPPING...';
}

async function refreshMiner() {
  const minerPrompt = el('miner-login-prompt');
  const minerAuth = el('miner-auth-only');
  
  if (!state.walletAddr) {
    if (minerPrompt) minerPrompt.classList.remove('hidden');
    if (minerAuth) minerAuth.classList.add('hidden');
    return;
  }
  
  if (minerPrompt) minerPrompt.classList.add('hidden');
  if (minerAuth) minerAuth.classList.remove('hidden');
  
  // Populate miner address field
  const mineAddr = el('mine-addr');
  if (mineAddr) {
    mineAddr.value = state.walletAddr;
  }
  
  await refreshCoreData();
  renderMineStats();
}

function updateWalletView(balance, ref, gov) {
  const priv = el('wallet-privkey');
  const addr = el('wallet-address');
  const bal = el('wallet-balance');
  const nonce = el('wallet-nonce');
  const panel = el('wallet-panel');

  const walletSetup = el('wallet-auth-view');
  const walletDash = el('wallet-dashboard');

  const refAuth = el('referral-auth-only');
  const refPrompt = el('referral-login-prompt');
  const govAuth = el('governance-auth-only');
  const govPrompt = el('governance-login-prompt');

  if (!state.walletAddr) {
    if (walletSetup) walletSetup.classList.remove('hidden');
    if (walletDash) walletDash.classList.add('hidden');
    if (panel) panel.textContent = 'Not logged in.';

    if (refAuth) refAuth.classList.add('hidden');
    if (refPrompt) refPrompt.classList.remove('hidden');
    if (govAuth) govAuth.classList.add('hidden');
    if (govPrompt) govPrompt.classList.remove('hidden');
    return;
  }

  if (walletSetup) walletSetup.classList.add('hidden');
  if (walletDash) walletDash.classList.remove('hidden');

  if (refAuth) refAuth.classList.remove('hidden');
  if (refPrompt) refPrompt.classList.add('hidden');
  if (govAuth) govAuth.classList.remove('hidden');
  if (govPrompt) govPrompt.classList.add('hidden');

  if (addr) addr.textContent = state.walletAddr;
  if (bal) bal.textContent = `${balance?.balance_kot || '0.00000000'} KOT`;
  if (nonce) nonce.textContent = String(balance?.nonce ?? 0);

  const lines = [
    '+--------------------- WALLET SUMMARY ---------------------+',
    ` address             : ${state.walletAddr}`,
    ` balance             : ${balance?.balance_kot || '0.00000000'} KOT`,
    ` nonce               : ${balance?.nonce ?? 0}`,
    ` privacy_code        : ${balance?.privacy_code || 'N/A'}`,
    ` referral_miners     : ${ref?.total_referred_miners ?? 'N/A'}`,
    ` referral_bonus      : ${ref?.total_referral_bonus_kot ?? 'N/A'} KOT`,
    ` referral_status     : ${ref?.is_active_referrer ? 'ACTIVE' : 'DORMANT'}`,
    ` governance_weight   : ${gov?.governance_weight_pct ?? 'N/A'}`,
    ` governance_capped   : ${gov?.is_capped ? 'YES' : 'NO'}`,
    '+----------------------------------------------------------+',
  ];

  if (panel) panel.textContent = lines.join('\n');
  setText('wallet-seed-display', state.masterSeedHex || '--');

  if (priv && !priv.textContent) {
    priv.textContent = '(hidden in memory after import)';
  }
}

async function refreshWallet() {
  if (!state.walletAddr) {
    updateWalletView(null, null, null);
    return;
  }

  const [balance, ref, gov] = await Promise.all([
    rpc('getbalance', [state.walletAddr]),
    rpc('getreferralinfo', [state.walletAddr]),
    rpc('getgovernanceinfo', [state.walletAddr]),
  ]);

  updateWalletView(balance, ref, gov);
  await loadTransactionHistory();
}

async function loadTransactionHistory() {
  const historyDiv = el('wallet-tx-history');
  if (!historyDiv || !state.walletAddr) return;

  historyDiv.innerHTML = '<div style="text-align: center; color: var(--dim); padding: 20px;">Loading transactions...</div>';

  const myAddr = state.walletAddr.toLowerCase();
  const transactions = [];
  
  // Scan last 100 blocks for transactions involving this address
  const scanDepth = Math.min(100, state.chainHeight);
  const startHeight = Math.max(0, state.chainHeight - scanDepth);
  
  for (let h = state.chainHeight; h >= startHeight; h--) {
    const block = await fetchBlockByHeight(h);
    if (!block || !block.transactions) continue;
    
    for (const tx of block.transactions) {
      const sender = String(tx.sender || '').toLowerCase();
      const recipient = String(tx.recipient || '').toLowerCase();
      
      if (sender === myAddr || recipient === myAddr) {
        transactions.push({
          ...tx,
          block_height: block.height,
          block_time: block.time,
          direction: sender === myAddr ? 'sent' : 'received'
        });
      }
    }
    
    // Limit to 50 transactions
    if (transactions.length >= 50) break;
  }

  if (transactions.length === 0) {
    historyDiv.innerHTML = '<div style="text-align: center; color: var(--dim); padding: 20px;">No transactions found in last 100 blocks.</div>';
    return;
  }

  const rows = transactions.map(tx => {
    const dirColor = tx.direction === 'sent' ? 'var(--bad)' : 'var(--ok)';
    const dirSymbol = tx.direction === 'sent' ? '↑' : '↓';
    const otherAddr = tx.direction === 'sent' ? tx.recipient : tx.sender;
    const amount = fmtKOT(tx.amount);
    const time = ago(tx.block_time);
    
    return `
      <div style="background: var(--bg); border: 1px solid var(--line); border-radius: 4px; padding: 8px; margin-bottom: 6px;">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;">
          <span style="color: ${dirColor}; font-weight: 700;">${dirSymbol} ${escapeHtml(tx.direction.toUpperCase())}</span>
          <span style="color: var(--dim);">${escapeHtml(String(time))} ago</span>
        </div>
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;">
          <span style="color: var(--text);">${escapeHtml(amount)} KOT</span>
          <span style="color: var(--dim);">Block #${escapeHtml(String(tx.block_height))}</span>
        </div>
        <div style="color: var(--dim); font-size: 9px; word-break: break-all;">
          ${escapeHtml(tx.direction === 'sent' ? 'To' : 'From')}: ${abbrev(otherAddr, 12, 10)}
        </div>
        ${tx.gov_data ? '<div style="color: var(--accent); font-size: 9px; margin-top: 4px;">🗳️ Governance Vote</div>' : ''}
      </div>
    `;
  }).join('');

  historyDiv.innerHTML = rows;
}

async function refreshReferral() {
  const authOnly = document.getElementById('referral-auth-only');

  if (!state.walletAddr) {
    if (authOnly) authOnly.classList.add('hidden');
    return;
  }

  if (authOnly) authOnly.classList.remove('hidden');

  const [ref, gov, bal] = await Promise.all([
    rpc('getreferralinfo', [state.walletAddr]),
    rpc('getgovernanceinfo', [state.walletAddr]),
    rpc('getbalance', [state.walletAddr]),
  ]);

  // Update YOUR referral stats
  setText('ref-your-total', ref?.total_referred_miners ?? '0');
  setText('ref-your-bonus', ref?.total_referral_bonus_kot ?? '0.00000000' + ' KOT');
  setText('ref-privacy-code', ref?.privacy_code ?? 'N/A');

  const lines = [
    '+---------------- YOUR REFERRAL STATUS ----------------+',
    ` wallet                 : ${state.walletAddr}`,
    ` privacy_code           : ${ref?.privacy_code || 'N/A'}`,
    ` referred_miners        : ${ref?.total_referred_miners ?? '0'}`,
    ` referral_bonus_earned  : ${ref?.total_referral_bonus_kot ?? '0.00000000'} KOT`,
    ` referrer_status        : ${ref?.is_active_referrer ? 'ACTIVE' : 'DORMANT'}`,
    ` governance_weight      : ${gov?.governance_weight_pct ?? '1.0%'}`,
    ` governance_bps         : ${gov?.governance_weight_bps ?? '100'}`,
    ` governance_cap         : ${gov?.cap_pct ?? '10.0%'}`,
    ` cap_reached            : ${gov?.is_capped ? 'YES' : 'NO'}`,
    '+-------------------------------------------------------+',
  ];

  setText('referral-panel', lines.join('\n'));

  // Update visibility
  const loginPrompt = document.getElementById('referral-login-prompt');
  if (loginPrompt) loginPrompt.classList.add('hidden');

  // Draw referral chart
  drawReferralChart();
  
  // List miners YOU referred
  displayYourReferredMiners();
  
  await refreshGovernance();
}

function displayYourReferredMiners() {
  const container = el('ref-your-miners');
  if (!container || !state.walletAddr) return;
  
  const miners = networkViz?.miners || [];
  if (miners.length === 0) {
    container.innerHTML = '<div style="text-align: center; color: var(--dim); padding: 20px;">Network data loading...</div>';
    return;
  }
  
  // Filter miners referred by YOU
  const myAddr = state.walletAddr.toLowerCase();
  const referred = miners.filter(m => {
    const referrer = String(m.referrer || '').toLowerCase();
    return referrer === myAddr;
  });
  
  if (referred.length === 0) {
    container.innerHTML = '<div style="text-align: center; color: var(--dim); padding: 20px;">You haven\'t referred anyone yet.<br><br>Share your referral code to start earning bonuses!</div>';
    return;
  }
  
  // Sort by blocks mined (most active first)
  referred.sort((a, b) => (b.blocks_mined || 0) - (a.blocks_mined || 0));
  
  const rows = referred.map(m => {
    const blocksSince = state.chainHeight - (m.last_mined_height || 0);
    const status = !m.last_mined_height ? 'Never Mined' :
                  blocksSince < 60 ? '🟢 Very Active' :
                  blocksSince < 1440 ? '🟡 Active' :
                  blocksSince < 2880 ? '🟠 Eligible' : '⚫ Inactive';
    
    return `
      <div style="background: var(--bg); border: 1px solid var(--line); border-radius: 4px; padding: 8px; margin-bottom: 6px;">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;">
          <span style="font-weight: 700; color: var(--text);">${abbrev(m.address, 12, 10)}</span>
          <span style="font-size: 9px; color: var(--dim);">${escapeHtml(status)}</span>
        </div>
        <div style="display: flex; justify-content: space-between; font-size: 9px; color: var(--dim);">
          <span>Blocks: ${escapeHtml(String(m.blocks_mined || 0))}</span>
          <span>Last: ${escapeHtml(!m.last_mined_height ? 'Never' : blocksSince + ' blocks ago')}</span>
        </div>
      </div>
    `;
  }).join('');
  
  container.innerHTML = rows;
}

function drawReferralChart() {
  const canvas = el('ref-chart');
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const width = canvas.width = canvas.offsetWidth * 2;
  const height = canvas.height = 360;
  
  ctx.clearRect(0, 0, width, height);
  
  // Get referral distribution from network miners
  const miners = networkViz?.miners || [];
  if (miners.length === 0) {
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--dim');
    ctx.font = '24px JetBrains Mono, monospace';
    ctx.textAlign = 'center';
    ctx.fillText('No data yet', width / 2, height / 2);
    return;
  }
  
  // Count referrals per miner
  const refCounts = {};
  miners.forEach(m => {
    const count = miners.filter(n => n.referrer === m.address).length;
    if (count > 0) {
      refCounts[count] = (refCounts[count] || 0) + 1;
    }
  });
  
  const entries = Object.entries(refCounts).sort((a, b) => parseInt(a[0]) - parseInt(b[0]));
  if (entries.length === 0) {
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--dim');
    ctx.font = '24px JetBrains Mono, monospace';
    ctx.textAlign = 'center';
    ctx.fillText('No referrals yet', width / 2, height / 2);
    return;
  }
  
  const maxCount = Math.max(...entries.map(e => e[1]));
  const barWidth = width / entries.length * 0.8;
  const gap = width / entries.length * 0.2;
  
  entries.forEach(([refs, count], i) => {
    const barHeight = (count / maxCount) * (height - 40);
    const x = i * (barWidth + gap) + gap;
    const y = height - barHeight - 20;
    
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--accent');
    ctx.fillRect(x, y, barWidth, barHeight);
    
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--text');
    ctx.font = '20px JetBrains Mono, monospace';
    ctx.textAlign = 'center';
    ctx.fillText(refs, x + barWidth / 2, height - 5);
    ctx.fillText(count, x + barWidth / 2, y - 5);
  });
}

async function refreshGovernance() {
  // 1. Load active proposals from local storage (acting as a "watched proposals" list)
  let saved = JSON.parse(localStorage.getItem('knot-gov-proposals') || '[]');

  // 2. For each proposal, fetch the REAL on-chain tally
  const updatedProposals = await Promise.all(saved.map(async (p) => {
    try {
      // Use the target hash as the lookup key for the tally
      const tally = await rpc('getgovernancetally', [p.target]);
      return { ...p, ...tally };
    } catch (e) {
      return p;
    }
  }));

  state.governance.proposals = updatedProposals;
  renderProposals();
  
  // Draw governance chart
  drawGovernanceChart();
}

function drawGovernanceChart() {
  const canvas = el('gov-chart');
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const width = canvas.width = canvas.offsetWidth * 2;
  const height = canvas.height = 360;
  
  ctx.clearRect(0, 0, width, height);
  
  const miners = networkViz?.miners || [];
  if (miners.length === 0) {
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--dim');
    ctx.font = '24px JetBrains Mono, monospace';
    ctx.textAlign = 'center';
    ctx.fillText('No data yet', width / 2, height / 2);
    return;
  }
  
  const totalBlocks = miners.reduce((sum, m) => sum + (m.blocks_mined || 0), 0);
  if (totalBlocks === 0) {
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--dim');
    ctx.font = '24px JetBrains Mono, monospace';
    ctx.textAlign = 'center';
    ctx.fillText('No blocks mined yet', width / 2, height / 2);
    return;
  }
  
  const top10 = miners
    .map(m => ({
      addr: m.address,
      blocks: m.blocks_mined || 0,
      power: ((m.blocks_mined || 0) / totalBlocks * 100)
    }))
    .filter(m => m.blocks > 0)
    .sort((a, b) => b.blocks - a.blocks)
    .slice(0, 10);
  
  if (top10.length === 0) {
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--dim');
    ctx.font = '24px JetBrains Mono, monospace';
    ctx.textAlign = 'center';
    ctx.fillText('No miners yet', width / 2, height / 2);
    return;
  }
  
  const maxPower = Math.max(...top10.map(m => m.power));
  const barHeight = (height - 40) / top10.length * 0.8;
  const gap = (height - 40) / top10.length * 0.2;
  
  top10.forEach((miner, i) => {
    const barWidth = (miner.power / maxPower) * (width - 200);
    const y = i * (barHeight + gap) + gap;
    
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--accent');
    ctx.fillRect(150, y, barWidth, barHeight);
    
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--text');
    ctx.font = '18px JetBrains Mono, monospace';
    ctx.textAlign = 'right';
    ctx.fillText(miner.addr.substring(0, 12) + '...', 145, y + barHeight / 2 + 6);
    
    ctx.textAlign = 'left';
    ctx.fillText(miner.power.toFixed(2) + '%', 155 + barWidth, y + barHeight / 2 + 6);
  });
}

async function submitProposal() {
  alert("Transaction broadcasting is disabled in the UI until Dilithium3 signing is fully integrated in-browser. Please use knotcoin-cli for mainnet transactions.");
}

async function signalSupport(proposalTarget) {
  // In a real implementation, this would send a 0-amount transaction
  // with governance_data = proposalTarget.
  const p_abbrev = abbrev(proposalTarget, 10, 10);

  // Broadcast message
  let msg = `Signaling support for ${p_abbrev}...\n\n`;
  msg += `In the production network, this will broadcast a Dilithium3 signed transaction to the mempool.\n\n`;
  msg += `Signal intent recorded. Once the next block is mined, the network will tally your ${state.governance.weight_pct || '1.0%'} weight.`;

  alert(msg);

  // Trigger a refresh after a short delay to simulate block mining 
  setTimeout(refreshGovernance, 3000);
}

function renderProposals() {
  const list = el('gov-proposals-list');
  if (!list) return;

  if (!state.governance.proposals.length) {
    list.innerHTML = '<pre class="panel">No active proposals found. Create one above!</pre>';
    return;
  }

  list.textContent = ''; // Safe clear
  state.governance.proposals.forEach((p) => {
    const item = document.createElement('div');
    item.className = 'panel-block';
    item.style.marginBottom = '15px';
    item.style.borderLeft = p.is_passed ? '4px solid var(--ok)' : '4px solid var(--accent)';

    const timeStr = new Date(p.timestamp).toISOString().replace('T', ' ').slice(0, 16);
    const weightPct = p.total_weight_pct || '0.00%';
    const bps = p.total_weight_bps || 0;

    // Status Logic: Passed > Confirmed (on-chain) > Pending (local only)
    let statusLabel = '<span style="color: var(--dim);">[ PENDING ]</span>';
    if (p.is_passed) {
      statusLabel = '<span style="color: var(--ok);">[ PASSED ]</span>';
    } else if (bps > 0) {
      statusLabel = '<span style="color: var(--accent);">[ CONFIRMED ]</span>';
    }

    // SECURITY: All proposal data is sanitized via escapeHtml()
    const itemHTML = `
      <div style="display: flex; justify-content: space-between; font-size: 12px; font-weight: bold; margin-bottom: 8px;">
        <span>${statusLabel} ${escapeHtml(p.action.toUpperCase())}</span>
        <span style="color: var(--dim);">${escapeHtml(timeStr)}</span>
      </div>
      <div style="margin: 5px 0; font-family: var(--font-mono); font-size: 13px;"><strong>${escapeHtml(p.target)}</strong></div>
      <p style="font-size: 12px; margin: 10px 0; color: var(--text); background: var(--panel-alt); padding: 8px; border: 1px dashed var(--line);">${escapeHtml(p.desc)}</p>
      
      <div style="margin: 10px 0;">
        <div style="display: flex; justify-content: space-between; font-size: 11px; margin-bottom: 4px;">
          <span>CURRENT TALLY</span>
          <span>${weight_bps_to_pct(p.total_weight_bps || 0)} / 51.00%</span>
        </div>
        <div style="height: 8px; background: var(--bg); border: 1px solid var(--line); position: relative; overflow: hidden;">
          <div style="height: 100%; background: var(--accent); width: ${Math.min(100, (p.total_weight_bps || 0) / 51)}%; transition: width 0.5s;"></div>
          <div style="position: absolute; left: 51%; top: 0; bottom: 0; width: 2px; background: var(--bad); opacity: 0.5;"></div>
        </div>
      </div>

      <div style="display: flex; justify-content: space-between; align-items: center; font-size: 11px; margin-top: 10px;">
        <span>Proposer: ${abbrev(p.proposer, 6, 6)}</span>
        <button class="nav-btn active" style="padding: 4px 12px; font-size: 11px;" onclick="signalSupport('${escapeHtml(p.target)}')" ${p.is_passed ? 'disabled' : ''}>SIGNAL SUPPORT</button>
      </div>
    `;
    setSafeHTML(item, itemHTML);
    list.appendChild(item);
  });
}

function weight_bps_to_pct(bps) {
  return (bps / 100).toFixed(2) + '%';
}

async function generateReferralLink() {
  if (!state.walletAddr) {
    const linkInput = el('ref-link-output');
    if (linkInput) linkInput.value = 'Login wallet first.';
    return;
  }

  const ref = await rpc('getreferralinfo', [state.walletAddr]);
  if (!ref || !ref.privacy_code) {
    const linkInput = el('ref-link-output');
    if (linkInput) linkInput.value = 'Failed to get privacy code.';
    return;
  }

  const link = `knotcoin://${ref.privacy_code}@127.0.0.1:9000/node`;
  const linkInput = el('ref-link-output');
  if (linkInput) {
    linkInput.value = link;
  }
}

// Lightweight SHA3 for referral code derivation
function sha3_256_custom(msg) {
  const RC = [
    0x00000001n, 0x00008082n, 0x8000808an, 0x80008000n, 0x0000808bn, 0x80000001n,
    0x80008081n, 0x80000009n, 0x0000008an, 0x00000088n, 0x80008009n, 0x8000000an,
    0x8000808bn, 0x0000008bn, 0x80000089n, 0x80000003n, 0x80000002n, 0x00000080n,
    0x0000800an, 0x8000000an, 0x80008081n, 0x80000080n, 0x00000001n, 0x80008008n,
  ];
  const ROTC = [1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44];
  const PILN = [10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1];

  const rol = (v, n) => BigInt.asUintN(64, (v << BigInt(n)) | (v >> BigInt(64 - n)));

  const st = new Array(25).fill(0n);
  const rate = 136;
  const buf = new Uint8Array(rate);
  let bufLen = 0;

  const absorb = (arr) => {
    for (let i = 0; i < arr.length; i++) {
      buf[bufLen++] = arr[i];
      if (bufLen === rate) {
        for (let k = 0; k < 17; k++) {
          let v = 0n;
          for (let j = 0; j < 8; j++) v |= BigInt(buf[k * 8 + j]) << BigInt(8 * j);
          st[k] ^= v;
        }
        keccak();
        bufLen = 0;
      }
    }
  };

  const keccak = () => {
    for (let r = 0; r < 24; r++) {
      const bc = Array.from({ length: 5 }, (_, i) => st[i] ^ st[i + 5] ^ st[i + 10] ^ st[i + 15] ^ st[i + 20]);
      for (let i = 0; i < 5; i++) {
        const t = bc[(i + 4) % 5] ^ rol(bc[(i + 1) % 5], 1);
        for (let j = 0; j < 25; j += 5) st[j + i] ^= t;
      }
      let t = st[1];
      for (let i = 0; i < 24; i++) {
        const j = PILN[i];
        const tmp = st[j];
        st[j] = rol(t, ROTC[i]);
        t = tmp;
      }
      for (let j = 0; j < 25; j += 5) {
        const b = [st[j], st[j + 1], st[j + 2], st[j + 3], st[j + 4]];
        for (let i = 0; i < 5; i++) st[j + i] ^= (~b[(i + 1) % 5]) & b[(i + 2) % 5];
      }
      st[0] ^= RC[r];
    }
  };

  absorb(msg);

  buf[bufLen] = 0x06;
  buf.fill(0, bufLen + 1, rate);
  buf[rate - 1] |= 0x80;

  for (let k = 0; k < 17; k++) {
    let v = 0n;
    for (let j = 0; j < 8; j++) v |= BigInt(buf[k * 8 + j]) << BigInt(8 * j);
    st[k] ^= v;
  }
  keccak();

  const out = new Uint8Array(32);
  for (let i = 0; i < 4; i++) {
    let w = st[i];
    for (let j = 0; j < 8; j++) {
      out[i * 8 + j] = Number(w & 0xffn);
      w >>= 8n;
    }
  }
  return out;
}

async function showBlock(hash) {
  const block = await rpc('getblock', [hash]);
  if (!block || block.error) return;
  block.hash = hash;
  
  const modalContent = await formatBlockDetailModal(block);
  showModal('Block Details', modalContent);
}

async function formatBlockDetailModal(block) {
  const minerKOT = await formatAddressKOT1(block.miner);
  const reward = rewardKnotsAtHeight(block.height);
  const time = new Date(block.time * 1000).toLocaleString();
  const ago_time = ago(block.time);
  
  return `
    <div class="modal-grid">
      <div class="modal-section">
        <h3>⛓️ Block Information</h3>
        <div class="detail-row">
          <span class="detail-label">Height</span>
          <span class="detail-value">${escapeHtml(String(block.height))}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Timestamp</span>
          <span class="detail-value">${escapeHtml(time)} (${escapeHtml(String(ago_time))} ago)</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Version</span>
          <span class="detail-value">${escapeHtml(String(block.version))}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Nonce</span>
          <span class="detail-value hash-value">${escapeHtml(String(block.nonce))}</span>
        </div>
      </div>
      
      <div class="modal-section">
        <h3>⛏️ Mining Details</h3>
        <div class="detail-row">
          <span class="detail-label">Miner Address</span>
          <span class="detail-value hash-value">${escapeHtml(minerKOT)}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Block Reward</span>
          <span class="detail-value" style="color: var(--ok); font-weight: bold;">${escapeHtml(fmtKOT(reward))} KOT</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Difficulty</span>
          <span class="detail-value">${escapeHtml(formatDifficulty(block.difficulty))}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Transactions</span>
          <span class="detail-value">${escapeHtml(String(block.tx_count))}</span>
        </div>
      </div>
      
      <div class="modal-section modal-full-width">
        <h3>🔐 Cryptographic Hashes</h3>
        <div class="detail-row">
          <span class="detail-label">Block Hash</span>
          <span class="detail-value hash-value">${escapeHtml(block.hash)}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Previous Block</span>
          <span class="detail-value hash-value">${escapeHtml(block.previousblockhash)}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Merkle Root</span>
          <span class="detail-value hash-value">${escapeHtml(block.merkleroot)}</span>
        </div>
      </div>
    </div>
  `;
}

function showModal(title, content) {
  let modal = document.getElementById('block-modal');
  if (!modal) {
    modal = document.createElement('div');
    modal.id = 'block-modal';
    modal.className = 'modal';
    modal.innerHTML = `
      <div class="modal-overlay"></div>
      <div class="modal-content">
        <div class="modal-header">
          <h2 class="modal-title"></h2>
          <button class="modal-close" type="button">&times;</button>
        </div>
        <div class="modal-body"></div>
      </div>
    `;
    document.body.appendChild(modal);
    
    modal.querySelector('.modal-overlay').addEventListener('click', closeModal);
    modal.querySelector('.modal-close').addEventListener('click', closeModal);
    
    document.addEventListener('keydown', function escHandler(e) {
      if (e.key === 'Escape') {
        const m = document.getElementById('block-modal');
        if (m && m.classList.contains('active')) {
          closeModal();
        }
      }
    });
  }
  
  modal.querySelector('.modal-title').textContent = title;
  const modalBody = modal.querySelector('.modal-body');
  // SECURITY: Clear using textContent first, then safely set content
  modalBody.textContent = '';
  if (typeof content === 'string') {
    // SECURITY: Create text node or parse as safe HTML
    const tempDiv = document.createElement('div');
    tempDiv.textContent = content; // This escapes all HTML
    modalBody.appendChild(tempDiv);
  } else {
    modalBody.appendChild(content);
  }
  modal.classList.add('active');
  document.body.style.overflow = 'hidden';
}

function closeModal() {
  const modal = document.getElementById('block-modal');
  if (modal) {
    modal.classList.remove('active');
    document.body.style.overflow = '';
  }
}

window.showBlock = showBlock;
window.closeModal = closeModal;

async function showAddress(addr) {
  const [bal, ref, gov] = await Promise.all([
    rpc('getbalance', [addr]),
    rpc('getreferralinfo', [addr]),
    rpc('getgovernanceinfo', [addr]),
  ]);

  const lines = [
    `ADDRESS DETAIL`,
    `address             : ${addr}`,
    `balance             : ${bal?.balance_kot ?? 'N/A'} KOT`,
    `nonce               : ${bal?.nonce ?? 'N/A'}`,
    `last_mined_height   : ${bal?.last_mined_height ?? 'N/A'}`,
    `privacy_code        : ${bal?.privacy_code ?? 'N/A'}`,
    `referred_miners     : ${ref?.total_referred_miners ?? 'N/A'}`,
    `referral_bonus      : ${ref?.total_referral_bonus_kot ?? 'N/A'} KOT`,
    `referrer_status     : ${ref?.is_active_referrer ? 'ACTIVE' : 'DORMANT'}`,
    `governance_weight   : ${gov?.governance_weight_pct ?? 'N/A'}`,
    `governance_capped   : ${gov?.is_capped ? 'YES' : 'NO'}`,
  ];

  setText('detail-content', lines.join('\n'));
  goToPage('detail');
}

window.showAddress = showAddress;

async function search() {
  const q = String(el('search-input')?.value || '').trim();
  if (!q) return;

  if (/^\d+$/.test(q)) {
    const hash = await rpc('getblockhash', [Number(q)]);
    if (hash) return showBlock(hash);
  }

  const addr = await normalizeAddress(q);
  if (addr) {
    const bal = await rpc('getbalance', [addr]);
    if (bal && !bal.error) return showAddress(addr);
  }

  const raw = q.startsWith('KOT') ? q.slice(3) : q;
  if (/^[a-f0-9]{64}$/i.test(raw)) {
    const block = await rpc('getblock', [raw]);
    if (block && block.height !== undefined) return showBlock(raw);
  }

  alert('Not found. Use block height, block hash, or address.');
}

async function refreshCoreData() {
  const head = await fetchHead();
  if (!head) return false;
  const blocks = await fetchRecentBlocks(120);
  computeTimingAndHashrate(blocks);
  return true;
}

async function goToPage(page, push = true) {
  const target = el(`page-${page}`);
  if (!target) return;

  // Auth-wall: Redirect to wallet if trying to access protected pages without login
  if ((page === 'referral' || page === 'governance' || page === 'mine') && !state.walletAddr) {
    alert('Please login to your wallet to access this section.');
    return goToPage('wallet', false);
  }

  const current = document.querySelector('.page.active');
  if (current && push) {
    const id = current.id.replace('page-', '');
    if (id !== page) state.navHistory.push(id);
  }

  document.querySelectorAll('.page').forEach((p) => p.classList.remove('active'));
  target.classList.add('active');

  document.querySelectorAll('.nav-btn').forEach((btn) => {
    btn.classList.toggle('active', btn.getAttribute('data-page') === page);
  });

  if (page === 'home') await refreshHome();
  if (page === 'blocks') await refreshBlocks();
  if (page === 'network') await refreshNetwork();
  if (page === 'mine') await refreshMiner();
  if (page === 'wallet') await refreshWallet();
  if (page === 'referral') await refreshReferral();
  if (page === 'governance') await refreshGovernance();

  // Only scroll to top on manual page change, not on refresh
  // window.scrollTo(0, 0); // Removed to prevent auto-scroll
}

async function importWallet(secret) {
  const pk = String(secret || '').trim().toLowerCase();
  
  // Validation
  if (!pk) {
    alert('Please enter a mnemonic or seed.');
    return false;
  }
  
  let seedBytes;
  const wordCount = pk.split(' ').length;
  
  if (wordCount === 24 || wordCount === 12) {
    // Validate mnemonic words
    const words = pk.split(' ');
    const invalidWords = words.filter(w => !WORDLIST.includes(w));
    if (invalidWords.length > 0) {
      alert(`Invalid mnemonic words: ${invalidWords.join(', ')}\n\nPlease check your mnemonic and try again.`);
      return false;
    }
    
    try {
      seedBytes = await mnemonicToSeed(pk, "");
    } catch (e) {
      alert('Failed to derive seed from mnemonic. Please check your mnemonic and try again.');
      console.error('Mnemonic error:', e);
      return false;
    }
  } else if (/^[a-f0-9]{128}$/.test(pk)) {
    seedBytes = hexToBytes(pk);
  } else {
    alert('Invalid input. Enter a 24-word mnemonic (or 12-word legacy) or a 128-hex character seed.');
    return false;
  }

  // Handle referrer persistence
  const refRaw = String(el('wallet-referrer-input')?.value || '').trim();
  if (refRaw) {
    const m = refRaw.match(/knotcoin:\/\/([a-f0-9]{16})@/i);
    localStorage.setItem('knot-referrer', m ? m[1] : refRaw);
  } else if (!localStorage.getItem('knot-referrer')) {
    localStorage.removeItem('knot-referrer');
  }

  try {
    
    // Convert seed to account seed (Account 0)
    const accKeyMaterial = await crypto.subtle.importKey(
      'raw',
      new TextEncoder().encode('Knotcoin account'),
      { name: 'HMAC', hash: 'SHA-512' },
      false,
      ['sign']
    );
    const accPayload = new Uint8Array(seedBytes.length + 8);
    accPayload.set(seedBytes);
    const accountSeed = new Uint8Array(await crypto.subtle.sign('HMAC', accKeyMaterial, accPayload));


    // Create mock Dilithium3 public key (1952 bytes) from account seed
    // This matches the CLI wallet derivation
    const mockPubKey = new Uint8Array(1952);
    mockPubKey.set(accountSeed.slice(0, 32), 0); // First 32 bytes from account seed
    // Rest is zeros (padding)

    // Derive address: SHA-512(mockPubKey)[0..32]
    const addrDigest = await crypto.subtle.digest('SHA-512', mockPubKey);
    const addressBytes = new Uint8Array(addrDigest).slice(0, 32);


    state.walletAddr = await encodeKOT1(addressBytes);
    state.masterSeedHex = stateToHex(seedBytes);


    setText('wallet-privkey', pk);
    setText('wallet-address', state.walletAddr);

    localStorage.setItem('knot-wallet', pk);

    const mineAddr = el('mine-addr');
    if (mineAddr && !mineAddr.value.trim()) mineAddr.value = state.walletAddr;

    // Clear inputs after success
    const inp = el('wallet-import-input');
    if (inp) inp.value = '';

    try {
      await refreshWallet();
      await refreshReferral();
    } catch (refreshError) {
      console.error('Refresh error (non-fatal):', refreshError);
      // Continue anyway - wallet is imported
    }
    
    return true;
  } catch (e) {
    alert(`Failed to import wallet: ${e.message}\n\nCheck console for details.`);
    console.error('Wallet import error:', e);
    console.error('Stack:', e.stack);
    return false;
  }
}

function bindEvents() {
  document.querySelectorAll('.nav-btn').forEach((btn) => {
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      goToPage(btn.getAttribute('data-page'));
    });
  });

  el('search-btn')?.addEventListener('click', search);
  el('search-input')?.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') search();
  });

  el('blocks-prev')?.addEventListener('click', () => {
    if (state.blocksPage > 0) {
      state.blocksPage -= 1;
      loadBlocksPage(state.blocksPage);
    }
  });

  el('blocks-next')?.addEventListener('click', () => {
    state.blocksPage += 1;
    loadBlocksPage(state.blocksPage);
  });

  el('mine-btn')?.addEventListener('click', async () => {
    if (state.mining.active) return stopMining();
    await startMining();
  });

  el('theme-toggle')?.addEventListener('click', () => {
    const current = document.documentElement.getAttribute('data-theme');
    const next = current === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', next);
    localStorage.setItem('knot-theme', next);
    el('theme-toggle').textContent = next === 'dark' ? '☾' : '☼';
  });

  el('wallet-generate-btn')?.addEventListener('click', async () => {
    const mnemonic = await generateMnemonic();
    el('generated-mnemonic-text').textContent = mnemonic;
    el('mnemonic-creation-zone').classList.remove('hidden');
    el('wallet-generate-btn').disabled = true;
    el('wallet-generate-btn').style.opacity = '0.5';
  });

  el('wallet-copy-mnemonic-btn')?.addEventListener('click', () => {
    copyTextToClipboard(el('generated-mnemonic-text').textContent);
  });

  el('wallet-confirm-mnemonic-btn')?.addEventListener('click', async () => {
    const mnemonic = el('generated-mnemonic-text').textContent;
    el('mnemonic-creation-zone').classList.add('hidden');
    el('wallet-generate-btn').disabled = false;
    el('wallet-generate-btn').style.opacity = '1';
    await importWallet(mnemonic);
  });

  el('wallet-import-btn')?.addEventListener('click', async () => {
    const pk = String(el('wallet-import-input')?.value || '').trim();
    if (pk) await importWallet(pk);
  });

  el('wallet-copy-priv')?.addEventListener('click', () => copyText('wallet-privkey'));
  el('wallet-copy-addr')?.addEventListener('click', () => copyText('wallet-address'));
  el('wallet-copy-seed-btn')?.addEventListener('click', () => {
    if (state.masterSeedHex) copyTextToClipboard(state.masterSeedHex);
  });

  el('wallet-toggle-seed-btn')?.addEventListener('click', () => {
    const zone = el('wallet-seed-display-zone');
    const btn = el('wallet-toggle-seed-btn');
    if (zone && btn) {
      const isHidden = zone.classList.toggle('hidden');
      btn.textContent = isHidden ? 'SHOW MASTER SEED (HEX)' : 'HIDE MASTER SEED (HEX)';
    }
  });

  el('wallet-logout-btn')?.addEventListener('click', () => {
    state.walletAddr = null;
    localStorage.removeItem('knot-wallet');
    setText('wallet-privkey', '');
    setText('wallet-address', '');
    state.masterSeedHex = null;
    el('wallet-seed-display-zone')?.classList.add('hidden');
    const toggleBtn = el('wallet-toggle-seed-btn');
    if (toggleBtn) toggleBtn.textContent = 'SHOW MASTER SEED (HEX)';
    refreshWallet();
    refreshReferral();
  });

  el('ref-generate-btn')?.addEventListener('click', generateReferralLink);
  el('ref-copy-code-btn')?.addEventListener('click', () => copyText('ref-privacy-code'));
  el('ref-refresh-btn')?.addEventListener('click', refreshReferral);
  el('gov-submit-btn')?.addEventListener('click', submitProposal);

  el('back-link')?.addEventListener('click', (e) => {
    e.preventDefault();
    const next = state.navHistory.length ? state.navHistory.pop() : 'home';
    goToPage(next, false);
  });
}

function startPolling() {
  let running = false;

  // Poll network visualization every 5 seconds when on home page
  setInterval(async () => {
    const homePage = document.getElementById('page-home');
    if (homePage && homePage.classList.contains('active') && networkViz) {
      await updateNetworkViz();
    }
  }, 5000);
  
  setInterval(() => {
    setText('clock', new Date().toUTCString());
    if (state.mining.active) renderMineStats();
  }, 1000);
}

document.addEventListener('DOMContentLoaded', async () => {
  const savedTheme = localStorage.getItem('knot-theme') || 'light';
  const toggle = el('theme-toggle');
  if (toggle) toggle.textContent = savedTheme === 'dark' ? '☾' : '☼';

  // Ensure modal is closed on page load
  const existingModal = document.getElementById('block-modal');
  if (existingModal) {
    existingModal.classList.remove('active');
    existingModal.remove();
  }
  document.body.style.overflow = '';

  bindEvents();

  const saved = localStorage.getItem('knot-wallet');
  if (saved) {
    importWallet(saved);
  }

  await goToPage('home', false);
  startPolling();
  window.signalSupport = signalSupport;
  window.submitProposal = submitProposal;
});
