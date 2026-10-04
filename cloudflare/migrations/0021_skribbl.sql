-- Daily Skribbl minigame: themes, daily assignments, drawings, votes, winners.

CREATE TABLE IF NOT EXISTS skribbl_themes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  theme TEXT NOT NULL UNIQUE,
  last_used_date TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO skribbl_themes (theme) VALUES
  ('Vintage Typewriter'),
  ('Hiking through a storm'),
  ('Nostalgia'),
  ('Floating island'),
  ('The smell of old books'),
  ('Astronaut eating spaghetti'),
  ('Time travel'),
  ('Golden Retriever in a raincoat'),
  ('Electric guitar'),
  ('Midnight snack'),
  ('Solitude'),
  ('Lighthouse in the fog'),
  ('Cozy cabin in winter'),
  ('Cyberpunk city street'),
  ('Flying turtle'),
  ('Haunted mansion'),
  ('Euphoria'),
  ('Hot air balloon festival'),
  ('Submarine exploring an underwater cave'),
  ('Dragon hoarding coffee beans'),
  ('Giant mushroom house'),
  ('Rollercoaster at sunset'),
  ('Deja vu'),
  ('Mechanical clockwork heart'),
  ('Cat wearing a wizard hat'),
  ('Raindrops on a window'),
  ('Space station garden'),
  ('Treehouse in Autumn'),
  ('Penguin riding a skateboard'),
  ('Sunset over a calm ocean'),
  ('Melancholy'),
  ('Robot learning to paint'),
  ('Treasure map'),
  ('Campfire under the stars'),
  ('Snorkeling with jellyfish'),
  ('Alien visiting a grocery store'),
  ('Overdue library book'),
  ('Envy'),
  ('Steampunk airship'),
  ('Baking fresh bread'),
  ('Lost temple in the jungle'),
  ('Neon bowling alley'),
  ('Squirrel hiding a diamond'),
  ('Thunderstorm over a desert'),
  ('Quiet coffee shop on a rainy day'),
  ('Hope'),
  ('Flying bicycle'),
  ('Ice cream cone melting on the sidewalk'),
  ('Chameleon on a rainbow'),
  ('Secret garden gate'),
  ('Playing Durak on a train'),
  ('Origami crane'),
  ('Parallel universe'),
  ('Sloth running a marathon'),
  ('Sleeping dragon'),
  ('Durak card game champion'),
  ('Midnight in Tokyo'),
  ('Chaos'),
  ('Lighthouse keeper'),
  ('Detective solving a mystery'),
  ('Lost in a corn maze'),
  ('A robot playing Durak'),
  ('Floating lantern festival'),
  ('Serenity'),
  ('Castle in the clouds'),
  ('Pirate ship in a storm'),
  ('Ferret wearing sunglasses'),
  ('Antique pocket watch'),
  ('Loser of a Durak match taking out the trash'),
  ('Solar eclipse'),
  ('Greenhouse full of carnivorous plants'),
  ('Alien disguised as a human'),
  ('Regret'),
  ('Vintage camera'),
  ('Owl wearing a graduation cap'),
  ('High stakes Durak game in a saloon'),
  ('Ferris wheel at night'),
  ('Bamboo forest'),
  ('Hamster building a fort'),
  ('Mysterious glowing door'),
  ('Nostalgic arcade game'),
  ('Overgrown ruins'),
  ('Curiosity'),
  ('Wizard brewing a potion'),
  ('Submarine periscope view'),
  ('Fox sleeping in the snow'),
  ('Galaxy inside a glass marble'),
  ('DJing at a penguin party'),
  ('Abandoned amusement park'),
  ('Betrayal'),
  ('Venetian masquerade ball'),
  ('Flying whale'),
  ('Monkey riding a unicycle'),
  ('Cozy window seat on a rainy day'),
  ('Deep sea diver discovering an artifact'),
  ('Time capsule'),
  ('Electric jellyfish'),
  ('Bear drinking boba tea'),
  ('Lost mitten in the snow'),
  ('Wanderlust');

CREATE TABLE IF NOT EXISTS skribbl_daily_themes (
  date TEXT PRIMARY KEY,
  theme_id INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (theme_id) REFERENCES skribbl_themes(id)
);

CREATE TABLE IF NOT EXISTS skribbl_drawings (
  id TEXT PRIMARY KEY,
  date TEXT NOT NULL,
  user_id TEXT NOT NULL,
  theme_id INTEGER NOT NULL,
  r2_object_key TEXT NOT NULL UNIQUE,
  mime_type TEXT NOT NULL DEFAULT 'image/webp',
  size_bytes INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (theme_id) REFERENCES skribbl_themes(id),
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_skribbl_drawings_theme_created
  ON skribbl_drawings(theme_id, created_at);
CREATE INDEX IF NOT EXISTS idx_skribbl_drawings_date_created
  ON skribbl_drawings(date, created_at);
CREATE INDEX IF NOT EXISTS idx_skribbl_drawings_user_date
  ON skribbl_drawings(user_id, date);

CREATE TABLE IF NOT EXISTS skribbl_votes (
  drawing_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  vote_value INTEGER NOT NULL CHECK (vote_value IN (-1, 1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (drawing_id, user_id),
  FOREIGN KEY (drawing_id) REFERENCES skribbl_drawings(id) ON DELETE CASCADE,
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_skribbl_votes_user ON skribbl_votes(user_id);

CREATE TABLE IF NOT EXISTS skribbl_daily_winners (
  date TEXT PRIMARY KEY,
  drawing_id TEXT,
  user_id TEXT,
  display_name TEXT,
  score INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
