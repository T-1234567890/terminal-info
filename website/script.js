/* =============================================
   Terminal Info — Landing Page Script
   ============================================= */

(function () {
  "use strict";

  /* ------------------------------------------
     Demo scenarios
  ------------------------------------------ */
  var demos = {
    dashboard: {
      title: "Dashboard",
      command: "tinfo",
      info: {
        heading: "Instant system overview",
        desc: "Run <code>tinfo</code> with no arguments to see the live dashboard — weather, time, network, CPU, and memory in a single glance.",
        bullets: [
          "Configurable widget order in <code>~/.tinfo/config.toml</code>",
          "Live mode updates in real time; freeze with <code>--freeze</code>",
          "Plugin widgets appear when a trusted plugin exposes <code>--widget</code>",
          "Set a location alias to resolve weather automatically",
        ],
      },
      lines: [
        { type: "out", cls: "term-box-border", text: "\u250C\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2510" },
        { type: "out", cls: "term-box-border term-title-txt", text: "\u2502         Terminal Info            \u2502" },
        { type: "out", cls: "term-box-border", text: "\u251C\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2524" },
        { type: "kv",  key: "\u2502 Location:  ", val: "Shenzhen               \u2502" },
        { type: "kv",  key: "\u2502 Weather:   ", val: "Clear sky, 20.3\xb0C      \u2502" },
        { type: "kv",  key: "\u2502 Time:      ", val: "2026-03-28 09:08:31    \u2502" },
        { type: "kv",  key: "\u2502 Network:   ", val: "203.0.113.42           \u2502" },
        { type: "kv",  key: "\u2502 CPU:       ", val: "19.3%                  \u2502" },
        { type: "kv",  key: "\u2502 Memory:    ", val: "16.2 / 24.0 GiB        \u2502" },
        { type: "out", cls: "term-box-border", text: "\u2514\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2518" },
      ],
    },

    weather: {
      title: "Weather",
      command: "tinfo weather now",
      info: {
        heading: "Real-time weather",
        desc: "Get current conditions for your configured location or any city on demand. Powered by Open-Meteo with IP-based location fallback.",
        bullets: [
          "<code>tinfo weather now &lt;city&gt;</code> — override location on the fly",
          "<code>tinfo weather hourly</code> — 24-hour forecast",
          "<code>tinfo weather alerts</code> — active weather alerts",
          "Location aliases: <code>tinfo weather home</code>",
        ],
      },
      lines: [
        { type: "out", cls: "term-label", text: "Weather \u2014 Shenzhen" },
        { type: "blank" },
        { type: "kv", key: "  Condition   ", val: "Clear sky" },
        { type: "kv", key: "  Temperature ", val: "20.3 \u00b0C" },
        { type: "kv", key: "  Feels like  ", val: "18.9 \u00b0C" },
        { type: "kv", key: "  Humidity    ", val: "62 %" },
        { type: "kv", key: "  Wind        ", val: "12 km/h SSW" },
        { type: "kv", key: "  UV index    ", val: "3" },
        { type: "blank" },
        { type: "kv", key: "  Sunrise     ", val: "06:24" },
        { type: "kv", key: "  Sunset      ", val: "18:47" },
      ],
    },

    diagnostic: {
      title: "Diagnostic",
      command: "tinfo diagnostic network",
      info: {
        heading: "Network diagnostics",
        desc: "Run a full suite of network health checks — DNS resolution, external ping, HTTP reachability, and IP inspection. Server mode adds deeper checks.",
        bullets: [
          "<code>tinfo diagnostic system</code> — CPU, memory, uptime",
          "<code>tinfo diagnostic performance</code> — load and I/O",
          "<code>tinfo diagnostic security</code> — server mode only",
          "<code>tinfo diagnostic full</code> — run everything at once",
        ],
      },
      lines: [
        { type: "out", cls: "term-label", text: "[Network Diagnostic]" },
        { type: "blank" },
        { type: "check", key: "  DNS resolution    ", val: "ok", good: true },
        { type: "check", key: "  External ping     ", val: "ok (google.com 18ms)", good: true },
        { type: "check", key: "  HTTP reachability ", val: "ok", good: true },
        { type: "check", key: "  Cloudflare ping   ", val: "ok (12ms)", good: true },
        { type: "blank" },
        { type: "kv", key: "  Public IP         ", val: "203.0.113.42" },
        { type: "kv", key: "  Local IP          ", val: "192.168.1.42" },
        { type: "kv", key: "  ISP               ", val: "China Unicom" },
        { type: "blank" },
        { type: "out", cls: "good", text: "  All network checks passed." },
      ],
    },

    productivity: {
      title: "Productivity",
      command: "tinfo timer start 25m",
      info: {
        heading: "Built-in productivity",
        desc: "tinfo includes a full productivity suite right in the terminal — timer, tasks, notes, reminders, and command history, all wired into the dashboard.",
        bullets: [
          "<code>tinfo task</code> — interactive task manager (7-day trash recovery)",
          "<code>tinfo note add &lt;text&gt;</code> — capture quick notes",
          "<code>tinfo remind 15m take a break</code> — dashboard reminders",
          "<code>tinfo history --limit 10</code> — recent command history",
        ],
      },
      lines: [
        { type: "out", cls: "term-label", text: "Timer started" },
        { type: "blank" },
        { type: "kv", key: "  Duration   ", val: "25 minutes" },
        { type: "kv", key: "  Remaining  ", val: "24:58" },
        { type: "kv", key: "  Status     ", val: "Running" },
        { type: "blank" },
        { type: "out", cls: "term-output", text: "  Press Ctrl+C to stop." },
        { type: "blank" },
        { type: "out", cls: "term-label", text: "Tasks (2 open)" },
        { type: "blank" },
        { type: "kv", key: "  [ ] ", val: "Review pull requests" },
        { type: "kv", key: "  [ ] ", val: "Write release notes" },
        { type: "kv", key: "  [x] ", val: "Set up tinfo" },
      ],
    },

    plugin: {
      title: "Plugin",
      command: "tinfo plugin install news",
      info: {
        heading: "Plugin management",
        desc: "Find, install, trust, and manage community plugins directly from the CLI. Every plugin install is verified against a Minisign signature from the registry.",
        bullets: [
          "<code>tinfo plugin search &lt;query&gt;</code> — find plugins",
          "<code>tinfo plugin browse</code> — visual browser on localhost",
          "<code>tinfo plugin trust &lt;name&gt;</code> — allow execution",
          "<code>tinfo plugin init hello</code> — scaffold your own plugin",
        ],
      },
      lines: [
        { type: "out", cls: "term-label",  text: "Installing \u2018news\u2019..." },
        { type: "blank" },
        { type: "check", key: "  Resolving registry    ", val: "ok", good: true },
        { type: "check", key: "  Downloading v0.1.0    ", val: "ok", good: true },
        { type: "check", key: "  Verifying signature   ", val: "ok", good: true },
        { type: "check", key: "  SHA-256 checksum      ", val: "ok", good: true },
        { type: "blank" },
        { type: "out", cls: "term-output", text: "  Installed to ~/.terminal-info/plugins/news/" },
        { type: "blank" },
        { type: "out", cls: "term-label",  text: "Next steps" },
        { type: "blank" },
        { type: "kv", key: "  1. ", val: "tinfo plugin trust news" },
        { type: "kv", key: "  2. ", val: "tinfo news" },
      ],
    },
  };

  /* ------------------------------------------
     Render demo info panel
  ------------------------------------------ */
  function renderInfo(key) {
    var d = demos[key];
    if (!d) return;
    var info = d.info;
    var html = '<h3>' + info.heading + '</h3><p>' + info.desc + '</p><ul>';
    for (var i = 0; i < info.bullets.length; i++) {
      html += '<li>' + info.bullets[i] + '</li>';
    }
    html += '</ul>';
    var el = document.getElementById("demo-info");
    if (el) el.innerHTML = html;
  }

  /* ------------------------------------------
     Build terminal HTML lines
  ------------------------------------------ */
  function buildLines(key) {
    var d = demos[key];
    if (!d) return "";
    var html = "";
    // Command line
    html += '<div class="term-line"><span class="term-ps">$</span><span class="term-cmd" id="demo-cmd-text"></span><span class="term-cursor" id="demo-cursor"></span></div>';
    // Output wrapper (hidden initially)
    html += '<div id="demo-output" style="opacity:0;transition:opacity 0.3s">';
    for (var i = 0; i < d.lines.length; i++) {
      var l = d.lines[i];
      if (l.type === "blank") {
        html += '<div class="term-line" style="height:0.6rem"></div>';
      } else if (l.type === "out") {
        html += '<div class="term-line"><span class="term-out ' + (l.cls || "") + '">' + escHtml(l.text) + '</span></div>';
      } else if (l.type === "kv") {
        html += '<div class="term-line"><span class="term-key">' + escHtml(l.key) + '</span><span class="term-val">' + escHtml(l.val) + '</span></div>';
      } else if (l.type === "check") {
        var cls = l.good ? "good" : "bad";
        html += '<div class="term-line"><span class="term-key">' + escHtml(l.key) + '</span><span class="term-out ' + cls + '">\u2713 ' + escHtml(l.val) + '</span></div>';
      }
    }
    html += '</div>';
    // Prompt after output
    html += '<div id="demo-end-prompt" style="display:none;margin-top:0.5rem"><div class="term-line"><span class="term-ps">$</span><span class="term-cursor"></span></div></div>';
    return html;
  }

  function escHtml(str) {
    return String(str)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  /* ------------------------------------------
     Typewriter animation
  ------------------------------------------ */
  var typeTimer = null;
  var showTimer = null;

  function clearTimers() {
    if (typeTimer) clearTimeout(typeTimer);
    if (showTimer) clearTimeout(showTimer);
    typeTimer = null;
    showTimer = null;
  }

  function typeCommand(text, el, cursor, callback) {
    var i = 0;
    el.textContent = "";
    function step() {
      if (i < text.length) {
        el.textContent += text[i];
        i++;
        typeTimer = setTimeout(step, 38 + Math.random() * 20);
      } else {
        if (cursor) cursor.style.display = "none";
        if (callback) showTimer = setTimeout(callback, 300);
      }
    }
    step();
  }

  function runDemo(key) {
    var d = demos[key];
    if (!d) return;
    clearTimers();

    var body = document.getElementById("demo-term-body");
    if (!body) return;

    body.innerHTML = buildLines(key);

    var cmdEl    = document.getElementById("demo-cmd-text");
    var cursor   = document.getElementById("demo-cursor");
    var output   = document.getElementById("demo-output");
    var endPrompt = document.getElementById("demo-end-prompt");

    if (!cmdEl) return;

    typeCommand(d.command, cmdEl, cursor, function () {
      if (output) {
        output.style.opacity = "1";
      }
      if (endPrompt) {
        showTimer = setTimeout(function () {
          endPrompt.style.display = "block";
        }, 200);
      }
    });
  }

  /* ------------------------------------------
     Demo tab switching
  ------------------------------------------ */
  function initDemoTabs() {
    var tabs = document.querySelectorAll(".demo-tab");
    if (!tabs.length) return;

    function activate(key) {
      tabs.forEach(function (t) {
        var active = t.getAttribute("data-demo") === key;
        t.classList.toggle("active", active);
        t.setAttribute("aria-selected", active ? "true" : "false");
      });
      renderInfo(key);
      runDemo(key);
    }

    tabs.forEach(function (tab) {
      tab.addEventListener("click", function () {
        activate(tab.getAttribute("data-demo"));
      });
    });

    // Init first demo
    activate("dashboard");
  }

  /* ------------------------------------------
     Install tab switching
  ------------------------------------------ */
  function initInstallTabs() {
    var tabs     = document.querySelectorAll(".install-tab");
    var contents = document.querySelectorAll(".install-content");

    tabs.forEach(function (tab) {
      tab.addEventListener("click", function () {
        var key = tab.getAttribute("data-install");

        tabs.forEach(function (t) { t.classList.remove("active"); });
        contents.forEach(function (c) { c.classList.remove("active"); });

        tab.classList.add("active");
        var target = document.getElementById("install-" + key);
        if (target) target.classList.add("active");
      });
    });
  }

  /* ------------------------------------------
     Copy-to-clipboard helpers
  ------------------------------------------ */
  function copyText(text, btn) {
    if (!navigator.clipboard) {
      // Fallback for older browsers
      var ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.opacity  = "0";
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
      flashBtn(btn);
      return;
    }
    navigator.clipboard.writeText(text).then(function () {
      flashBtn(btn);
    });
  }

  function flashBtn(btn) {
    var original = btn.textContent;
    btn.textContent = "Copied!";
    btn.classList.add("copied");
    setTimeout(function () {
      btn.textContent = original;
      btn.classList.remove("copied");
    }, 2000);
  }

  function initCopyButtons() {
    // Hero install copy
    var heroBtn = document.getElementById("hero-copy-btn");
    var heroCmd = document.getElementById("hero-install-cmd");
    if (heroBtn && heroCmd) {
      heroBtn.addEventListener("click", function () {
        copyText(heroCmd.textContent.trim(), heroBtn);
      });
    }

    // Code block copy buttons
    document.querySelectorAll(".code-copy").forEach(function (btn) {
      btn.addEventListener("click", function () {
        var text = btn.getAttribute("data-copy") || "";
        copyText(text, btn);
      });
    });
  }

  /* ------------------------------------------
     Mobile hamburger menu
  ------------------------------------------ */
  function initHamburger() {
    var btn = document.getElementById("hamburger");
    var nav = document.getElementById("site-nav");
    if (!btn || !nav) return;

    btn.addEventListener("click", function () {
      var open = nav.classList.toggle("open");
      btn.setAttribute("aria-expanded", open ? "true" : "false");
    });

    // Close on nav link click
    nav.querySelectorAll("a").forEach(function (link) {
      link.addEventListener("click", function () {
        nav.classList.remove("open");
      });
    });
  }

  /* ------------------------------------------
     Smooth scroll for anchor links
  ------------------------------------------ */
  function initSmoothScroll() {
    document.querySelectorAll('a[href^="#"]').forEach(function (a) {
      a.addEventListener("click", function (e) {
        var id = a.getAttribute("href").slice(1);
        var target = document.getElementById(id);
        if (target) {
          e.preventDefault();
          var offset = 70; // header height
          var top = target.getBoundingClientRect().top + window.pageYOffset - offset;
          window.scrollTo({ top: top, behavior: "smooth" });
        }
      });
    });
  }

  /* ------------------------------------------
     Init
  ------------------------------------------ */
  document.addEventListener("DOMContentLoaded", function () {
    initDemoTabs();
    initInstallTabs();
    initCopyButtons();
    initHamburger();
    initSmoothScroll();
  });
})();
