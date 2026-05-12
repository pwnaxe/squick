const express = require("express");
const app = express();
const router = express.Router();

function listUsers(req, res) {
  res.json([]);
}

app.get("/api/users", listUsers);
app.post("/api/users", (req, res) => res.status(201).json({}));
app.put("/api/users/:id", (req, res) => res.json({}));

router.get("/products", (req, res) => res.json([]));
router.delete("/products/:id", (req, res) => res.sendStatus(204));

app.use("/v2", router);

app.listen(3000);
