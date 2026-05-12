import React from "react";
import { Navbar } from "./Navbar";

export function App() {
  return (
    <div>
      <Navbar user="alice" />
      <main>Hello world</main>
    </div>
  );
}

export function Layout() {
  return <Navbar />;
}
