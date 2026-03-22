import { render } from "solid-js/web";
import App from "./App";
import "./styles/app.css";

// Disable default browser context menu
document.addEventListener("contextmenu", (e) => e.preventDefault());

render(() => <App />, document.getElementById("root")!);
