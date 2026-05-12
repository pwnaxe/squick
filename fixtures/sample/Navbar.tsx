import React, { useState } from "react";
import { Link } from "react-router-dom";

/** Top navigation bar with mobile menu toggle. */
export function Navbar(props: { user?: string }) {
  const [open, setOpen] = useState(false);
  return (
    <nav className="navbar">
      <header>
        <Link to="/">Home</Link>
        <Link to="/about">About</Link>
      </header>
      {open && <aside>menu</aside>}
    </nav>
  );
}

// Click handler for the toggle button.
const handleToggle = () => setOpen((s) => !s);

interface NavbarProps {
  user?: string;
}

type Theme = "light" | "dark";
