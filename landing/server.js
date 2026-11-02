const express = require("express");
const path = require("path");

const app = express();
const PORT = process.env.PORT || 3000;

// Serve static files from /app
app.use(express.static("/app"));

// Fallback to index.html
app.get("*", (_req, res) => {
  res.sendFile(path.join("/app", "index.html"));
});

app.listen(PORT, "0.0.0.0", () => {
  console.log(`Polygone landing on http://0.0.0.0:${PORT}`);
});
