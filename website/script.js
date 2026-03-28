/* =============================================
   Terminal Info — Landing Page Script
   ============================================= */

(function () {
  "use strict";

  var RELEASE_API = "https://api.github.com/repos/T-1234567890/terminal-info/releases/latest";
  var VERSION_CACHE_KEY = "tinfo-version";
  var VERSION_CACHE_TTL = 10 * 60 * 1000;

  function normalizeVersion(tag) {
    if (!tag || typeof tag !== "string") return null;
    var value = tag.trim();
    return value || null;
  }

  function readVersionCache() {
    try {
      var raw = window.localStorage.getItem(VERSION_CACHE_KEY);
      if (!raw) return null;
      var parsed = JSON.parse(raw);
      if (!parsed || typeof parsed.version !== "string" || typeof parsed.cachedAt !== "number") {
        return null;
      }
      if (Date.now() - parsed.cachedAt > VERSION_CACHE_TTL) {
        return null;
      }
      return parsed;
    } catch (_err) {
      return null;
    }
  }

  function writeVersionCache(version, releaseUrl) {
    try {
      window.localStorage.setItem(
        VERSION_CACHE_KEY,
        JSON.stringify({
          version: version,
          releaseUrl: releaseUrl || null,
          cachedAt: Date.now()
        })
      );
    } catch (_err) {
      // Ignore cache failures and continue.
    }
  }

  function updateVersionDisplays(version, releaseUrl) {
    if (!version) return;
    document.querySelectorAll("[data-stable-version]").forEach(function (node) {
      node.textContent = version;
      if (releaseUrl && node.tagName === "A") {
        node.setAttribute("href", releaseUrl);
      }
    });
  }

  function initSectionReveal() {
    var nodes = document.querySelectorAll(".section, .stats-strip, .site-footer, [data-reveal]");
    if (!nodes.length) return;

    if (!("IntersectionObserver" in window)) {
      nodes.forEach(function (node) {
        node.classList.add("is-visible");
      });
      return;
    }

    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          entry.target.classList.add("is-visible");
          observer.unobserve(entry.target);
        }
      });
    }, {
      threshold: 0.12,
      rootMargin: "0px 0px -24px 0px"
    });

    nodes.forEach(function (node) {
      observer.observe(node);
    });
  }

  function animateValue(el, nextText) {
    if (!el || el.textContent === nextText) return;
    el.style.opacity = "0.35";
    el.style.transform = "translateY(5px)";
    window.setTimeout(function () {
      el.textContent = nextText;
      el.style.opacity = "1";
      el.style.transform = "translateY(0)";
    }, 220);
  }

  function pad(value) {
    return String(value).padStart(2, "0");
  }

  function padEnd(value, width) {
    var text = String(value);
    if (text.length >= width) return text.slice(0, width);
    return text + new Array(width - text.length + 1).join(" ");
  }

  function formatBoxRows(title, rows, options) {
    var opts = options || {};
    var width = opts.width || 34;
    var labelWidth = opts.labelWidth || 10;
    var innerWidth = width;

    function border(left, fill, right) {
      return left + new Array(innerWidth + 1).join(fill) + right;
    }

    function contentLine(text) {
      return "│" + padEnd(text, innerWidth) + "│";
    }

    var centeredTitle = title;
    var leftPad = Math.max(0, Math.floor((innerWidth - title.length) / 2));
    centeredTitle = new Array(leftPad + 1).join(" ") + title;
    centeredTitle = padEnd(centeredTitle, innerWidth);

    var lines = [
      border("┌", "─", "┐"),
      contentLine(centeredTitle),
      border("├", "─", "┤")
    ];

    rows.forEach(function (row) {
      var label = padEnd(row.label + ":", labelWidth);
      var text = " " + label + " " + row.value;
      lines.push(contentLine(text));
    });

    lines.push(border("└", "─", "┘"));
    return lines;
  }

  function renderBoxLine(line) {
    return '<div class="term-line"><span class="term-box">' + escHtml(line) + "</span></div>";
  }

  function heroRowsFromState(state) {
    return [
      { label: "Location", value: "Tokyo" },
      { label: "Weather", value: "Clear sky, 20.3°C" },
      { label: "Time", value: state.time },
      { label: "Network", value: "143.xxx.x.xx" },
      { label: "CPU", value: state.cpu },
      { label: "Memory", value: state.memory + " used" },
      { label: "Timers", value: state.timer },
      { label: "Reminders", value: state.reminder }
    ];
  }

  function renderHeroTerminal(state) {
    var body = document.getElementById("hero-term-body");
    if (!body) return;
    var lines = formatBoxRows("Terminal Info", heroRowsFromState(state), {
      width: 44,
      labelWidth: 10
    });
    var html = '<div class="term-line"><span class="term-ps">$</span><span class="term-cmd">tinfo</span></div>';
    lines.forEach(function (line) {
      html += renderBoxLine(line);
    });
    html += '<div class="term-line" style="height:0.6rem"></div>';
    html += '<div class="term-line"><span class="term-out">Press q or Ctrl+C to exit.</span></div>';
    body.innerHTML = html;
  }

  function initHeroTerminalUpdates() {
    var body = document.getElementById("hero-term-body");
    if (!body) return;

    var cpuSeries = ["28.4%", "31.2%", "35.6%", "42.1%", "51.8%"];
    var memorySeries = [
      "16.2 GiB / 24.0 GiB",
      "16.3 GiB / 24.0 GiB",
      "16.1 GiB / 24.0 GiB",
      "16.2 GiB / 24.0 GiB"
    ];
    var cpuIndex = 0;
    var memoryIndex = 0;
    var reminderTarget = Date.now() + 12 * 60 * 1000;
    var timerRemaining = 24 * 60 + 52;
    var state = {
      time: "2026-03-28 20:45:44",
      cpu: cpuSeries[cpuIndex],
      memory: memorySeries[memoryIndex],
      timer: formatCountdown(timerRemaining),
      reminder: formatReminder(reminderTarget)
    };

    renderHeroTerminal(state);

    window.setInterval(function () {
      var now = new Date();
      state.time = [
        now.getFullYear(),
        "-",
        pad(now.getMonth() + 1),
        "-",
        pad(now.getDate()),
        " ",
        pad(now.getHours()),
        ":",
        pad(now.getMinutes()),
        ":",
        pad(now.getSeconds())
      ].join("");
      if (timerRemaining > 0) {
        timerRemaining -= 1;
      }
      state.timer = formatCountdown(timerRemaining);
      state.reminder = formatReminder(reminderTarget);
      renderHeroTerminal(state);
    }, 1000);

    window.setInterval(function () {
      cpuIndex = (cpuIndex + 1) % cpuSeries.length;
      state.cpu = cpuSeries[cpuIndex];
      renderHeroTerminal(state);
    }, 4200);

    window.setInterval(function () {
      memoryIndex = (memoryIndex + 1) % memorySeries.length;
      state.memory = memorySeries[memoryIndex];
      renderHeroTerminal(state);
    }, 6200);
  }

  async function loadStableVersion() {
    var cached = readVersionCache();
    if (cached) {
      updateVersionDisplays(cached.version, cached.releaseUrl);
    }

    if (!window.fetch) {
      return cached ? cached.version : null;
    }

    try {
      var response = await window.fetch(RELEASE_API, {
        headers: {
          Accept: "application/vnd.github+json"
        }
      });
      if (!response.ok) {
        throw new Error("GitHub release lookup failed");
      }

      var payload = await response.json();
      var version = normalizeVersion(payload && payload.tag_name);
      var releaseUrl = payload && payload.html_url ? String(payload.html_url) : null;

      if (!version) {
        throw new Error("Latest release tag missing");
      }

      updateVersionDisplays(version, releaseUrl);
      writeVersionCache(version, releaseUrl);
      return version;
    } catch (_err) {
      if (cached) {
        updateVersionDisplays(cached.version, cached.releaseUrl);
        return cached.version;
      }
      return null;
    }
  }

  /* ------------------------------------------
     Demo scenarios
  ------------------------------------------ */
  var demos = {
    dashboard: {
      title: "Dashboard",
      command: "tinfo",
      info: {
        heading: "Instant system overview",
        bullets: [
          "Shows location, weather, time, network, CPU, and memory in one view",
          "Timers and reminders appear directly in the main summary",
          "Useful as a quick status check when you open the tool",
        ],
      },
      lines: [
        {
          type: "box",
          title: "Terminal Info",
          width: 44,
          labelWidth: 10,
          rows: [
            { label: "Location", value: "Tokyo" },
            { label: "Weather", value: "Clear sky, 20.3°C" },
            { label: "Time", value: "__DYNAMIC_TIME__" },
            { label: "Network", value: "143.xxx.x.xx" },
            { label: "CPU", value: "19.3%" },
            { label: "Memory", value: "16.2 GiB / 24.0 GiB used" },
            { label: "Timers", value: "Timer: 00:24:52 remaining" },
            { label: "Reminders", value: "⏳ break in 12 min" }
          ]
        },
        { type: "blank" },
        { type: "out", cls: "term-output", text: "Press q or Ctrl+C to exit." }
      ],
    },

    weather: {
      title: "Weather",
      command: "tinfo weather now",
      info: {
        heading: "Real-time weather",
        bullets: [
          "Shows the current conditions in a compact boxed view",
          "Includes temperature, wind, and humidity",
          "Useful when you want a quick weather check without leaving the terminal",
        ],
      },
      lines: [
        {
          type: "box",
          title: "Tokyo Weather",
          width: 38,
          labelWidth: 12,
          rows: [
            { label: "Location", value: "Tokyo" },
            { label: "Weather", value: "Clear sky" },
            { label: "Temperature", value: "20.3°C" },
            { label: "Wind", value: "3.1 m/s" },
            { label: "Humidity", value: "61%" }
          ]
        }
      ],
    },

    diagnostic: {
      title: "Diagnostic",
      command: "tinfo diagnostic network",
      info: {
        heading: "Network diagnostics",
        bullets: [
          "Checks DNS, ping, HTTP reachability, and Cloudflare connectivity",
          "Also reports public IP, local IP, and ISP details",
          "Useful when you want a fast connectivity check from the CLI",
        ],
      },
      lines: [
        { type: "check", key: "  DNS resolution     ", val: "ok", good: true },
        { type: "check", key: "  External ping      ", val: "26 ms", good: true },
        { type: "check", key: "  HTTP reachability  ", val: "ok", good: true },
        { type: "check", key: "  Cloudflare ping    ", val: "14 ms", good: true },
        { type: "kv", key: "  Public IP          ", val: "143.xxx.x.xx" },
        { type: "kv", key: "  Local IP           ", val: "192.168.x.xx" },
        { type: "kv", key: "  ISP                ", val: "Example ISP" },
        { type: "blank" },
        { type: "out", cls: "good", text: "  All network checks passed." },
      ],
    },

    productivity: {
      title: "Productivity",
      command: "tinfo timer start 25m",
      info: {
        heading: "Built-in productivity",
        bullets: [
          "Starts a countdown timer from the command line",
          "Shows the remaining time in a compact live view",
          "Useful for focused work sessions directly in the terminal",
        ],
      },
      lines: [
        { type: "out", cls: "term-output", text: "Started countdown timer for 25m 0s." },
        { type: "blank" },
        {
          type: "box",
          title: "Terminal Info Timer",
          width: 27,
          labelWidth: 7,
          rows: [
            { label: "Timer", value: "__DYNAMIC_TIMER__" }
          ]
        },
        { type: "blank" },
        { type: "out", cls: "term-output", text: "Press q or Ctrl+C to exit." }
      ],
    },

    plugin: {
      title: "Plugin",
      command: "tinfo plugin install news",
      info: {
        heading: "Plugin management",
        bullets: [
          "The install command resolves a plugin name and places the binary in the managed plugin directory",
          "Installed plugins become available as first-class terminal commands",
          "Useful for extending the CLI without modifying the core tool",
        ],
      },
      lines: [
        { type: "out", cls: "good", text: "Installed plugin 'news' at /Users/you/.terminal-info/plugins/docker/tinfo-news." },
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
    var html = '<h3>' + info.heading + '</h3>';
    if (info.desc) {
      html += '<p>' + info.desc + '</p>';
    }
    if (info.bullets && info.bullets.length) {
      html += '<ul>';
      for (var i = 0; i < info.bullets.length; i++) {
        html += '<li>' + info.bullets[i] + '</li>';
      }
      html += '</ul>';
    }
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
      } else if (l.type === "box") {
        if (
          l.rows &&
          l.rows.some(function (row) {
            return row.value === "__DYNAMIC_TIME__";
          })
        ) {
          html += '<div data-dynamic-box="dashboard"></div>';
          continue;
        }
        if (
          l.rows &&
          l.rows.length === 1 &&
          l.rows[0].label === "Timer" &&
          l.rows[0].value === "__DYNAMIC_TIMER__"
        ) {
          html += '<div data-dynamic-box="timer"></div>';
          continue;
        }
        var boxLines = formatBoxRows(l.title, l.rows, {
          width: l.width,
          labelWidth: l.labelWidth
        });
        for (var j = 0; j < boxLines.length; j++) {
          html += renderBoxLine(boxLines[j]);
        }
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
  var demoValueTimer = null;
  var demoClockTimer = null;

  function clearTimers() {
    if (typeTimer) clearTimeout(typeTimer);
    if (showTimer) clearTimeout(showTimer);
    if (demoValueTimer) clearInterval(demoValueTimer);
    if (demoClockTimer) clearInterval(demoClockTimer);
    typeTimer = null;
    showTimer = null;
    demoValueTimer = null;
    demoClockTimer = null;
  }

  function formatDateTime(now) {
    return [
      now.getFullYear(),
      "-",
      pad(now.getMonth() + 1),
      "-",
      pad(now.getDate()),
      " ",
      pad(now.getHours()),
      ":",
      pad(now.getMinutes()),
      ":",
      pad(now.getSeconds())
    ].join("");
  }

  function formatCountdown(totalSeconds) {
    var hours = Math.floor(totalSeconds / 3600);
    var minutes = Math.floor((totalSeconds % 3600) / 60);
    var seconds = totalSeconds % 60;
    return pad(hours) + ":" + pad(minutes) + ":" + pad(seconds) + " remaining";
  }

  function formatReminder(targetTime) {
    var diffMs = Math.max(0, targetTime - Date.now());
    var remainingMinutes = Math.ceil(diffMs / 60000);
    if (remainingMinutes <= 0) {
      return "break due now";
    }
    if (remainingMinutes === 1) {
      return "break in 1 min";
    }
    return "break in " + remainingMinutes + " min";
  }

  function startProductivityDemo() {
    var output = document.getElementById("demo-output");
    if (!output) return;

    var remaining = 25 * 60;

    function render() {
      var lines = formatBoxRows("Terminal Info Timer", [
        { label: "Timer", value: formatCountdown(remaining) }
      ], {
        width: 27,
        labelWidth: 7
      });

      var html = "";
      for (var i = 0; i < lines.length; i++) {
        html += renderBoxLine(lines[i]);
      }

      output.querySelectorAll('[data-dynamic-box="timer"]').forEach(function (node) {
        node.remove();
      });

      var wrapper = document.createElement("div");
      wrapper.setAttribute("data-dynamic-box", "timer");
      wrapper.innerHTML = html;
      output.appendChild(wrapper);
    }

    render();
    demoValueTimer = window.setInterval(function () {
      if (remaining > 0) {
        remaining -= 1;
      } else {
        clearInterval(demoValueTimer);
        demoValueTimer = null;
        var endPrompt = document.getElementById("demo-end-prompt");
        if (endPrompt) {
          endPrompt.style.display = "block";
        }
      }
      render();
    }, 1000);
  }

  function startDashboardDemo() {
    var output = document.getElementById("demo-output");
    if (!output) return;

    var cpuSeries = ["19.3%", "22.1%", "24.8%", "21.7%"];
    var memorySeries = [
      "16.2 GiB / 24.0 GiB used",
      "16.3 GiB / 24.0 GiB used",
      "16.1 GiB / 24.0 GiB used",
      "16.2 GiB / 24.0 GiB used"
    ];
    var reminderTarget = Date.now() + 12 * 60 * 1000;
    var timerRemaining = 24 * 60 + 52;
    var cpuIndex = 0;

    function render() {
      var now = new Date();
      var lines = formatBoxRows("Terminal Info", [
        { label: "Location", value: "Tokyo" },
        { label: "Weather", value: "Clear sky, 20.3°C" },
        { label: "Time", value: formatDateTime(now) },
        { label: "Network", value: "143.xxx.x.xx" },
        { label: "CPU", value: cpuSeries[cpuIndex] },
        { label: "Memory", value: memorySeries[cpuIndex] },
        { label: "Timers", value: formatCountdown(timerRemaining) },
        { label: "Reminders", value: formatReminder(reminderTarget) }
      ], {
        width: 44,
        labelWidth: 10
      });
      var html = "";
      for (var i = 0; i < lines.length; i++) {
        html += renderBoxLine(lines[i]);
      }

      output.querySelectorAll('[data-dynamic-box="dashboard"]').forEach(function (node) {
        node.remove();
      });

      var wrapper = document.createElement("div");
      wrapper.setAttribute("data-dynamic-box", "dashboard");
      wrapper.innerHTML = html;
      output.insertBefore(wrapper, output.firstChild);
    }

    render();
    demoClockTimer = window.setInterval(function () {
      cpuIndex = (cpuIndex + 1) % cpuSeries.length;
      if (timerRemaining > 0) {
        timerRemaining -= 1;
      }
      render();
    }, 1000);
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
      if (endPrompt && key !== "dashboard" && key !== "productivity") {
        showTimer = setTimeout(function () {
          endPrompt.style.display = "block";
        }, 200);
      }
      if (key === "dashboard") {
        startDashboardDemo();
      }
      if (key === "productivity") {
        startProductivityDemo();
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
        copyText(heroCmd.getAttribute("data-copy") || heroCmd.textContent.trim(), heroBtn);
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
    loadStableVersion();
    initSectionReveal();
    initHeroTerminalUpdates();
    initDemoTabs();
    initInstallTabs();
    initCopyButtons();
    initHamburger();
    initSmoothScroll();
  });
})();
