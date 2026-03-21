//! Built-in agent registry bundled with Loop Commander.
//!
//! These agents are available immediately at install time without network access.
//! The registry can still be refreshed from GitHub for updates, but the app always
//! ships with this complete list as the baseline.

use crate::prompt::AgentEntry;

/// Return the full built-in agent registry.
///
/// This list mirrors the awesome-claude-code-subagents catalog. Each entry
/// includes a slug, human-readable name, one-line description, and category.
pub fn builtin_agents() -> Vec<AgentEntry> {
    vec![
        // ── Core Development ────────────────────────────────
        AgentEntry {
            slug: "fullstack-developer".into(),
            name: "Fullstack Developer".into(),
            description: "Builds complete features spanning database, API, and frontend layers together as a cohesive unit".into(),
            category: "core-development".into(),
        },
        AgentEntry {
            slug: "backend-developer".into(),
            name: "Backend Developer".into(),
            description: "Builds server-side APIs, microservices, and backend systems with robust architecture and scalability".into(),
            category: "core-development".into(),
        },
        AgentEntry {
            slug: "frontend-developer".into(),
            name: "Frontend Developer".into(),
            description: "Builds complete frontend applications across React, Vue, and Angular frameworks".into(),
            category: "core-development".into(),
        },
        AgentEntry {
            slug: "mobile-developer".into(),
            name: "Mobile Developer".into(),
            description: "Builds cross-platform mobile applications with native performance and offline-first architecture".into(),
            category: "core-development".into(),
        },
        AgentEntry {
            slug: "mobile-app-developer".into(),
            name: "Mobile App Developer".into(),
            description: "Develops iOS and Android mobile applications with platform-specific UX optimization".into(),
            category: "core-development".into(),
        },
        AgentEntry {
            slug: "api-designer".into(),
            name: "API Designer".into(),
            description: "Designs APIs, creates OpenAPI specifications, and architects scalable API systems".into(),
            category: "core-development".into(),
        },
        AgentEntry {
            slug: "graphql-architect".into(),
            name: "GraphQL Architect".into(),
            description: "Designs GraphQL schemas, federation architectures, and optimizes query performance".into(),
            category: "core-development".into(),
        },
        AgentEntry {
            slug: "websocket-engineer".into(),
            name: "WebSocket Engineer".into(),
            description: "Implements real-time bidirectional communication features using WebSockets at scale".into(),
            category: "core-development".into(),
        },
        AgentEntry {
            slug: "microservices-architect".into(),
            name: "Microservices Architect".into(),
            description: "Designs distributed system architecture and decomposes monoliths into microservices".into(),
            category: "core-development".into(),
        },

        // ── Language Specialists ────────────────────────────
        AgentEntry {
            slug: "rust-engineer".into(),
            name: "Rust Engineer".into(),
            description: "Builds Rust systems with memory safety, ownership patterns, and zero-cost abstractions".into(),
            category: "language-specialists".into(),
        },
        AgentEntry {
            slug: "python-pro".into(),
            name: "Python Pro".into(),
            description: "Builds type-safe, production-ready Python code for web APIs and complex applications".into(),
            category: "language-specialists".into(),
        },
        AgentEntry {
            slug: "typescript-pro".into(),
            name: "TypeScript Pro".into(),
            description: "Implements advanced TypeScript type system patterns and end-to-end type safety".into(),
            category: "language-specialists".into(),
        },
        AgentEntry {
            slug: "javascript-pro".into(),
            name: "JavaScript Pro".into(),
            description: "Builds and optimizes modern JavaScript for browser, Node.js, or full-stack applications".into(),
            category: "language-specialists".into(),
        },
        AgentEntry {
            slug: "golang-pro".into(),
            name: "Go Pro".into(),
            description: "Builds Go applications with concurrent programming and cloud-native architectures".into(),
            category: "language-specialists".into(),
        },
        AgentEntry {
            slug: "java-architect".into(),
            name: "Java Architect".into(),
            description: "Designs enterprise Java architectures and microservices patterns for cloud-native systems".into(),
            category: "language-specialists".into(),
        },
        AgentEntry {
            slug: "csharp-developer".into(),
            name: "C# Developer".into(),
            description: "Builds ASP.NET Core web APIs and modern C# applications with clean architecture".into(),
            category: "language-specialists".into(),
        },
        AgentEntry {
            slug: "cpp-pro".into(),
            name: "C++ Pro".into(),
            description: "Builds high-performance C++ systems with modern C++20/23 features and zero-overhead abstractions".into(),
            category: "language-specialists".into(),
        },
        AgentEntry {
            slug: "swift-expert".into(),
            name: "Swift Expert".into(),
            description: "Builds native iOS, macOS, and server-side Swift applications with advanced concurrency".into(),
            category: "language-specialists".into(),
        },
        AgentEntry {
            slug: "kotlin-specialist".into(),
            name: "Kotlin Specialist".into(),
            description: "Builds Kotlin applications with advanced coroutine patterns and multiplatform code sharing".into(),
            category: "language-specialists".into(),
        },
        AgentEntry {
            slug: "php-pro".into(),
            name: "PHP Pro".into(),
            description: "Works with PHP 8.3+ projects with strict typing and enterprise framework expertise".into(),
            category: "language-specialists".into(),
        },
        AgentEntry {
            slug: "elixir-expert".into(),
            name: "Elixir Expert".into(),
            description: "Builds fault-tolerant, concurrent systems with OTP patterns and Phoenix framework".into(),
            category: "language-specialists".into(),
        },

        // ── Frameworks ──────────────────────────────────────
        AgentEntry {
            slug: "react-specialist".into(),
            name: "React Specialist".into(),
            description: "Optimizes React applications with advanced React 18+ features and state management".into(),
            category: "frameworks".into(),
        },
        AgentEntry {
            slug: "nextjs-developer".into(),
            name: "Next.js Developer".into(),
            description: "Builds production Next.js 14+ applications with App Router and server components".into(),
            category: "frameworks".into(),
        },
        AgentEntry {
            slug: "vue-expert".into(),
            name: "Vue Expert".into(),
            description: "Builds Vue 3 applications with Composition API mastery and Nuxt 3 development".into(),
            category: "frameworks".into(),
        },
        AgentEntry {
            slug: "angular-architect".into(),
            name: "Angular Architect".into(),
            description: "Architects enterprise Angular 15+ applications with complex state management".into(),
            category: "frameworks".into(),
        },
        AgentEntry {
            slug: "django-developer".into(),
            name: "Django Developer".into(),
            description: "Builds Django 4+ web applications and REST APIs with async views".into(),
            category: "frameworks".into(),
        },
        AgentEntry {
            slug: "rails-expert".into(),
            name: "Rails Expert".into(),
            description: "Builds Rails applications with Hotwire reactivity and real-time features".into(),
            category: "frameworks".into(),
        },
        AgentEntry {
            slug: "spring-boot-engineer".into(),
            name: "Spring Boot Engineer".into(),
            description: "Builds enterprise Spring Boot 3+ applications with microservices architecture".into(),
            category: "frameworks".into(),
        },
        AgentEntry {
            slug: "laravel-specialist".into(),
            name: "Laravel Specialist".into(),
            description: "Builds Laravel 10+ applications with Eloquent, queue systems, and API optimization".into(),
            category: "frameworks".into(),
        },
        AgentEntry {
            slug: "flutter-expert".into(),
            name: "Flutter Expert".into(),
            description: "Builds cross-platform mobile applications with Flutter 3+ and complex state management".into(),
            category: "frameworks".into(),
        },
        AgentEntry {
            slug: "electron-pro".into(),
            name: "Electron Pro".into(),
            description: "Builds Electron desktop applications with native OS integration and security hardening".into(),
            category: "frameworks".into(),
        },
        AgentEntry {
            slug: "dotnet-core-expert".into(),
            name: ".NET Core Expert".into(),
            description: "Builds .NET Core applications with cloud-native architecture and high-performance microservices".into(),
            category: "frameworks".into(),
        },
        AgentEntry {
            slug: "dotnet-framework-4-8-expert".into(),
            name: ".NET Framework 4.8 Expert".into(),
            description: "Maintains and modernizes legacy .NET Framework 4.8 enterprise applications".into(),
            category: "frameworks".into(),
        },

        // ── Infrastructure & DevOps ─────────────────────────
        AgentEntry {
            slug: "devops-engineer".into(),
            name: "DevOps Engineer".into(),
            description: "Builds infrastructure automation, CI/CD pipelines, and deployment workflows".into(),
            category: "infrastructure".into(),
        },
        AgentEntry {
            slug: "cloud-architect".into(),
            name: "Cloud Architect".into(),
            description: "Designs and optimizes cloud infrastructure architecture at scale".into(),
            category: "infrastructure".into(),
        },
        AgentEntry {
            slug: "kubernetes-specialist".into(),
            name: "Kubernetes Specialist".into(),
            description: "Designs, deploys, and troubleshoots Kubernetes clusters and workloads".into(),
            category: "infrastructure".into(),
        },
        AgentEntry {
            slug: "docker-expert".into(),
            name: "Docker Expert".into(),
            description: "Builds, optimizes, and secures Docker container images and orchestration".into(),
            category: "infrastructure".into(),
        },
        AgentEntry {
            slug: "terraform-engineer".into(),
            name: "Terraform Engineer".into(),
            description: "Builds and scales infrastructure as code with multi-cloud deployments".into(),
            category: "infrastructure".into(),
        },
        AgentEntry {
            slug: "terragrunt-expert".into(),
            name: "Terragrunt Expert".into(),
            description: "Masters infrastructure orchestration with DRY configurations and multi-environment deployments".into(),
            category: "infrastructure".into(),
        },
        AgentEntry {
            slug: "azure-infra-engineer".into(),
            name: "Azure Infra Engineer".into(),
            description: "Designs and manages Azure infrastructure with Entra ID, PowerShell, and Bicep IaC".into(),
            category: "infrastructure".into(),
        },
        AgentEntry {
            slug: "platform-engineer".into(),
            name: "Platform Engineer".into(),
            description: "Builds internal developer platforms with self-service infrastructure and golden paths".into(),
            category: "infrastructure".into(),
        },
        AgentEntry {
            slug: "network-engineer".into(),
            name: "Network Engineer".into(),
            description: "Designs and troubleshoots cloud and hybrid network infrastructures".into(),
            category: "infrastructure".into(),
        },
        AgentEntry {
            slug: "sre-engineer".into(),
            name: "SRE Engineer".into(),
            description: "Establishes system reliability through SLO definition, error budgets, and automation".into(),
            category: "infrastructure".into(),
        },
        AgentEntry {
            slug: "deployment-engineer".into(),
            name: "Deployment Engineer".into(),
            description: "Designs and optimizes CI/CD pipelines and deployment automation strategies".into(),
            category: "infrastructure".into(),
        },
        AgentEntry {
            slug: "build-engineer".into(),
            name: "Build Engineer".into(),
            description: "Optimizes build performance, reduces compilation times, and scales build systems".into(),
            category: "infrastructure".into(),
        },

        // ── Database ────────────────────────────────────────
        AgentEntry {
            slug: "database-administrator".into(),
            name: "Database Administrator".into(),
            description: "Optimizes database performance and implements high-availability architectures".into(),
            category: "database".into(),
        },
        AgentEntry {
            slug: "database-optimizer".into(),
            name: "Database Optimizer".into(),
            description: "Analyzes slow queries and implements indexing strategies across database systems".into(),
            category: "database".into(),
        },
        AgentEntry {
            slug: "sql-pro".into(),
            name: "SQL Pro".into(),
            description: "Optimizes complex SQL queries and designs efficient schemas across PostgreSQL, MySQL, SQL Server".into(),
            category: "database".into(),
        },
        AgentEntry {
            slug: "postgres-pro".into(),
            name: "PostgreSQL Pro".into(),
            description: "Optimizes PostgreSQL performance, designs replication, and troubleshoots at scale".into(),
            category: "database".into(),
        },

        // ── Quality & Security ──────────────────────────────
        AgentEntry {
            slug: "code-reviewer".into(),
            name: "Code Reviewer".into(),
            description: "Conducts comprehensive code reviews focusing on quality, security, and best practices".into(),
            category: "quality-security".into(),
        },
        AgentEntry {
            slug: "security-engineer".into(),
            name: "Security Engineer".into(),
            description: "Implements security solutions, builds automated controls into CI/CD, and manages vulnerabilities".into(),
            category: "quality-security".into(),
        },
        AgentEntry {
            slug: "security-auditor".into(),
            name: "Security Auditor".into(),
            description: "Conducts comprehensive security audits, compliance assessments, and risk evaluations".into(),
            category: "quality-security".into(),
        },
        AgentEntry {
            slug: "penetration-tester".into(),
            name: "Penetration Tester".into(),
            description: "Conducts authorized security penetration tests to identify real vulnerabilities".into(),
            category: "quality-security".into(),
        },
        AgentEntry {
            slug: "test-automator".into(),
            name: "Test Automator".into(),
            description: "Builds automated test frameworks, creates test scripts, and integrates testing into CI/CD".into(),
            category: "quality-security".into(),
        },
        AgentEntry {
            slug: "qa-expert".into(),
            name: "QA Expert".into(),
            description: "Comprehensive quality assurance strategy and test planning across the development cycle".into(),
            category: "quality-security".into(),
        },
        AgentEntry {
            slug: "accessibility-tester".into(),
            name: "Accessibility Tester".into(),
            description: "Comprehensive accessibility testing, WCAG compliance, and assistive technology support".into(),
            category: "quality-security".into(),
        },
        AgentEntry {
            slug: "compliance-auditor".into(),
            name: "Compliance Auditor".into(),
            description: "Achieves regulatory compliance across GDPR, HIPAA, PCI DSS, SOC 2, and ISO standards".into(),
            category: "quality-security".into(),
        },

        // ── Debugging & Performance ─────────────────────────
        AgentEntry {
            slug: "debugger".into(),
            name: "Debugger".into(),
            description: "Diagnoses and fixes bugs, identifies root causes, and analyzes error logs".into(),
            category: "debugging-performance".into(),
        },
        AgentEntry {
            slug: "error-detective".into(),
            name: "Error Detective".into(),
            description: "Diagnoses errors, correlates across services, identifies root causes, and prevents failures".into(),
            category: "debugging-performance".into(),
        },
        AgentEntry {
            slug: "performance-engineer".into(),
            name: "Performance Engineer".into(),
            description: "Identifies and eliminates performance bottlenecks in applications and infrastructure".into(),
            category: "debugging-performance".into(),
        },
        AgentEntry {
            slug: "refactoring-specialist".into(),
            name: "Refactoring Specialist".into(),
            description: "Transforms poorly structured code into clean, maintainable systems while preserving behavior".into(),
            category: "debugging-performance".into(),
        },
        AgentEntry {
            slug: "legacy-modernizer".into(),
            name: "Legacy Modernizer".into(),
            description: "Modernizes legacy systems with incremental migration strategies and risk mitigation".into(),
            category: "debugging-performance".into(),
        },
        AgentEntry {
            slug: "dependency-manager".into(),
            name: "Dependency Manager".into(),
            description: "Audits dependencies for vulnerabilities, resolves conflicts, and optimizes bundle sizes".into(),
            category: "debugging-performance".into(),
        },

        // ── Data & AI ───────────────────────────────────────
        AgentEntry {
            slug: "data-engineer".into(),
            name: "Data Engineer".into(),
            description: "Designs and optimizes data pipelines, ETL/ELT processes, and data infrastructure".into(),
            category: "data-ai".into(),
        },
        AgentEntry {
            slug: "data-scientist".into(),
            name: "Data Scientist".into(),
            description: "Analyzes data patterns, builds predictive models, and extracts statistical insights".into(),
            category: "data-ai".into(),
        },
        AgentEntry {
            slug: "data-analyst".into(),
            name: "Data Analyst".into(),
            description: "Extracts insights from business data, creates dashboards, and performs statistical analysis".into(),
            category: "data-ai".into(),
        },
        AgentEntry {
            slug: "ml-engineer".into(),
            name: "ML Engineer".into(),
            description: "Builds production ML systems with model training pipelines and serving infrastructure".into(),
            category: "data-ai".into(),
        },
        AgentEntry {
            slug: "mlops-engineer".into(),
            name: "MLOps Engineer".into(),
            description: "Designs ML infrastructure, CI/CD for models, and automated training pipelines".into(),
            category: "data-ai".into(),
        },
        AgentEntry {
            slug: "ai-engineer".into(),
            name: "AI Engineer".into(),
            description: "Architects end-to-end AI systems from model selection to production deployment".into(),
            category: "data-ai".into(),
        },
        AgentEntry {
            slug: "llm-architect".into(),
            name: "LLM Architect".into(),
            description: "Designs LLM systems for production, implements RAG architectures, and optimizes inference".into(),
            category: "data-ai".into(),
        },
        AgentEntry {
            slug: "nlp-engineer".into(),
            name: "NLP Engineer".into(),
            description: "Builds production NLP systems and text processing pipelines for domain-specific tasks".into(),
            category: "data-ai".into(),
        },
        AgentEntry {
            slug: "prompt-engineer".into(),
            name: "Prompt Engineer".into(),
            description: "Designs, optimizes, tests, and evaluates prompts for LLMs in production systems".into(),
            category: "data-ai".into(),
        },

        // ── Developer Experience ────────────────────────────
        AgentEntry {
            slug: "cli-developer".into(),
            name: "CLI Developer".into(),
            description: "Builds command-line tools with intuitive design and cross-platform compatibility".into(),
            category: "developer-experience".into(),
        },
        AgentEntry {
            slug: "tooling-engineer".into(),
            name: "Tooling Engineer".into(),
            description: "Builds developer tools including CLIs, code generators, build tools, and IDE extensions".into(),
            category: "developer-experience".into(),
        },
        AgentEntry {
            slug: "dx-optimizer".into(),
            name: "DX Optimizer".into(),
            description: "Optimizes developer workflow including build times, feedback loops, and testing efficiency".into(),
            category: "developer-experience".into(),
        },
        AgentEntry {
            slug: "git-workflow-manager".into(),
            name: "Git Workflow Manager".into(),
            description: "Designs and optimizes Git workflows, branching strategies, and merge management".into(),
            category: "developer-experience".into(),
        },
        AgentEntry {
            slug: "mcp-developer".into(),
            name: "MCP Developer".into(),
            description: "Builds and optimizes Model Context Protocol servers and clients for AI tool integration".into(),
            category: "developer-experience".into(),
        },

        // ── Documentation ───────────────────────────────────
        AgentEntry {
            slug: "technical-writer".into(),
            name: "Technical Writer".into(),
            description: "Creates and maintains technical documentation including API references and user guides".into(),
            category: "documentation".into(),
        },
        AgentEntry {
            slug: "documentation-engineer".into(),
            name: "Documentation Engineer".into(),
            description: "Architects comprehensive documentation systems that keep pace with code changes".into(),
            category: "documentation".into(),
        },
        AgentEntry {
            slug: "api-documenter".into(),
            name: "API Documenter".into(),
            description: "Creates API documentation, OpenAPI specifications, and interactive documentation portals".into(),
            category: "documentation".into(),
        },

        // ── Design & UX ─────────────────────────────────────
        AgentEntry {
            slug: "ui-designer".into(),
            name: "UI Designer".into(),
            description: "Designs visual interfaces, component libraries, and design systems with accessibility".into(),
            category: "design-ux".into(),
        },
        AgentEntry {
            slug: "ux-researcher".into(),
            name: "UX Researcher".into(),
            description: "Conducts user research, analyzes behavior, and generates actionable design insights".into(),
            category: "design-ux".into(),
        },

        // ── Architecture & Review ───────────────────────────
        AgentEntry {
            slug: "architect-reviewer".into(),
            name: "Architect Reviewer".into(),
            description: "Evaluates system designs, architectural patterns, and technology choices at the macro level".into(),
            category: "architecture".into(),
        },

        // ── Incident & Operations ───────────────────────────
        AgentEntry {
            slug: "incident-responder".into(),
            name: "Incident Responder".into(),
            description: "Responds to security breaches and service outages with evidence preservation and recovery".into(),
            category: "operations".into(),
        },
        AgentEntry {
            slug: "devops-incident-responder".into(),
            name: "DevOps Incident Responder".into(),
            description: "Responds to production incidents, diagnoses critical failures, and conducts postmortems".into(),
            category: "operations".into(),
        },
        AgentEntry {
            slug: "chaos-engineer".into(),
            name: "Chaos Engineer".into(),
            description: "Designs controlled failure experiments and validates system resilience before incidents".into(),
            category: "operations".into(),
        },
        AgentEntry {
            slug: "performance-monitor".into(),
            name: "Performance Monitor".into(),
            description: "Establishes observability infrastructure to track metrics and detect anomalies".into(),
            category: "operations".into(),
        },

        // ── Windows & PowerShell ────────────────────────────
        AgentEntry {
            slug: "powershell-5-1-expert".into(),
            name: "PowerShell 5.1 Expert".into(),
            description: "Automates Windows infrastructure with RSAT modules for AD, DNS, DHCP, GPO management".into(),
            category: "windows".into(),
        },
        AgentEntry {
            slug: "powershell-7-expert".into(),
            name: "PowerShell 7 Expert".into(),
            description: "Builds cross-platform cloud automation and Azure orchestration with PowerShell 7+".into(),
            category: "windows".into(),
        },
        AgentEntry {
            slug: "powershell-module-architect".into(),
            name: "PowerShell Module Architect".into(),
            description: "Architects PowerShell modules, profile systems, and cross-version automation libraries".into(),
            category: "windows".into(),
        },
        AgentEntry {
            slug: "powershell-security-hardening".into(),
            name: "PowerShell Security Hardening".into(),
            description: "Hardens PowerShell automation, secures remoting, and enforces least-privilege design".into(),
            category: "windows".into(),
        },
        AgentEntry {
            slug: "powershell-ui-architect".into(),
            name: "PowerShell UI Architect".into(),
            description: "Designs desktop GUIs (WinForms, WPF) and terminal UIs for PowerShell automation tools".into(),
            category: "windows".into(),
        },
        AgentEntry {
            slug: "windows-infra-admin".into(),
            name: "Windows Infra Admin".into(),
            description: "Manages Windows Server, Active Directory, DNS, DHCP, and Group Policy configurations".into(),
            category: "windows".into(),
        },
        AgentEntry {
            slug: "ad-security-reviewer".into(),
            name: "AD Security Reviewer".into(),
            description: "Audits Active Directory security posture and evaluates privilege escalation risks".into(),
            category: "windows".into(),
        },
        AgentEntry {
            slug: "m365-admin".into(),
            name: "M365 Admin".into(),
            description: "Automates Microsoft 365 admin tasks including Exchange, Teams, SharePoint, and Graph API".into(),
            category: "windows".into(),
        },
        AgentEntry {
            slug: "it-ops-orchestrator".into(),
            name: "IT Ops Orchestrator".into(),
            description: "Orchestrates complex IT operations spanning PowerShell, .NET, Azure, and M365".into(),
            category: "windows".into(),
        },

        // ── Specialized Domains ─────────────────────────────
        AgentEntry {
            slug: "game-developer".into(),
            name: "Game Developer".into(),
            description: "Implements game systems, optimizes rendering, and builds multiplayer networking".into(),
            category: "specialized".into(),
        },
        AgentEntry {
            slug: "embedded-systems".into(),
            name: "Embedded Systems".into(),
            description: "Develops firmware for resource-constrained microcontrollers with RTOS and real-time guarantees".into(),
            category: "specialized".into(),
        },
        AgentEntry {
            slug: "iot-engineer".into(),
            name: "IoT Engineer".into(),
            description: "Designs IoT solutions with device management, edge computing, and cloud integration".into(),
            category: "specialized".into(),
        },
        AgentEntry {
            slug: "blockchain-developer".into(),
            name: "Blockchain Developer".into(),
            description: "Builds smart contracts, DApps, and blockchain protocols with security auditing".into(),
            category: "specialized".into(),
        },
        AgentEntry {
            slug: "payment-integration".into(),
            name: "Payment Integration".into(),
            description: "Implements payment systems with PCI compliance, fraud prevention, and secure processing".into(),
            category: "specialized".into(),
        },
        AgentEntry {
            slug: "fintech-engineer".into(),
            name: "Fintech Engineer".into(),
            description: "Builds payment systems and financial integrations with regulatory compliance".into(),
            category: "specialized".into(),
        },
        AgentEntry {
            slug: "slack-expert".into(),
            name: "Slack Expert".into(),
            description: "Develops Slack applications, implements Slack API integrations, and reviews bot security".into(),
            category: "specialized".into(),
        },
        AgentEntry {
            slug: "wordpress-master".into(),
            name: "WordPress Master".into(),
            description: "Architects WordPress implementations from custom themes to enterprise multisite platforms".into(),
            category: "specialized".into(),
        },

        // ── Business & Product ──────────────────────────────
        AgentEntry {
            slug: "product-manager".into(),
            name: "Product Manager".into(),
            description: "Makes product strategy decisions, prioritizes features, and defines roadmap plans".into(),
            category: "business".into(),
        },
        AgentEntry {
            slug: "project-manager".into(),
            name: "Project Manager".into(),
            description: "Establishes project plans, tracks execution, manages risks, and coordinates stakeholders".into(),
            category: "business".into(),
        },
        AgentEntry {
            slug: "scrum-master".into(),
            name: "Scrum Master".into(),
            description: "Facilitates agile ceremonies, optimizes velocity, and scales agile practices across teams".into(),
            category: "business".into(),
        },
        AgentEntry {
            slug: "business-analyst".into(),
            name: "Business Analyst".into(),
            description: "Analyzes business processes, gathers requirements, and identifies improvement opportunities".into(),
            category: "business".into(),
        },
        AgentEntry {
            slug: "legal-advisor".into(),
            name: "Legal Advisor".into(),
            description: "Drafts contracts, reviews compliance, develops IP protection, and assesses legal risks".into(),
            category: "business".into(),
        },
        AgentEntry {
            slug: "risk-manager".into(),
            name: "Risk Manager".into(),
            description: "Identifies, quantifies, and mitigates enterprise risks across financial and operational domains".into(),
            category: "business".into(),
        },
        AgentEntry {
            slug: "customer-success-manager".into(),
            name: "Customer Success Manager".into(),
            description: "Assesses customer health, develops retention strategies, and maximizes lifetime value".into(),
            category: "business".into(),
        },
        AgentEntry {
            slug: "sales-engineer".into(),
            name: "Sales Engineer".into(),
            description: "Conducts technical pre-sales including solution architecture and proof-of-concept development".into(),
            category: "business".into(),
        },

        // ── Research & Analysis ─────────────────────────────
        AgentEntry {
            slug: "research-analyst".into(),
            name: "Research Analyst".into(),
            description: "Comprehensive research across multiple sources with synthesis into actionable insights".into(),
            category: "research".into(),
        },
        AgentEntry {
            slug: "data-researcher".into(),
            name: "Data Researcher".into(),
            description: "Discovers, collects, and validates data from multiple sources for analysis".into(),
            category: "research".into(),
        },
        AgentEntry {
            slug: "market-researcher".into(),
            name: "Market Researcher".into(),
            description: "Analyzes markets, consumer behavior, competitive landscapes, and sizes opportunities".into(),
            category: "research".into(),
        },
        AgentEntry {
            slug: "competitive-analyst".into(),
            name: "Competitive Analyst".into(),
            description: "Analyzes competitors, benchmarks against market leaders, and develops positioning strategies".into(),
            category: "research".into(),
        },
        AgentEntry {
            slug: "trend-analyst".into(),
            name: "Trend Analyst".into(),
            description: "Analyzes emerging patterns and predicts industry shifts for strategic planning".into(),
            category: "research".into(),
        },
        AgentEntry {
            slug: "search-specialist".into(),
            name: "Search Specialist".into(),
            description: "Finds specific information across sources using advanced search strategies and query optimization".into(),
            category: "research".into(),
        },
        AgentEntry {
            slug: "quant-analyst".into(),
            name: "Quant Analyst".into(),
            description: "Develops quantitative trading strategies, financial models, and derivatives pricing".into(),
            category: "research".into(),
        },

        // ── Marketing & Content ─────────────────────────────
        AgentEntry {
            slug: "content-marketer".into(),
            name: "Content Marketer".into(),
            description: "Develops content strategies, creates SEO-optimized marketing content, and drives engagement".into(),
            category: "marketing".into(),
        },
        AgentEntry {
            slug: "seo-specialist".into(),
            name: "SEO Specialist".into(),
            description: "Comprehensive SEO optimization including technical audits, keyword strategy, and rankings".into(),
            category: "marketing".into(),
        },

        // ── Orchestration & Meta ────────────────────────────
        AgentEntry {
            slug: "agent-organizer".into(),
            name: "Agent Organizer".into(),
            description: "Assembles and optimizes multi-agent teams for complex projects with task decomposition".into(),
            category: "orchestration".into(),
        },
        AgentEntry {
            slug: "multi-agent-coordinator".into(),
            name: "Multi-Agent Coordinator".into(),
            description: "Coordinates concurrent agents with communication, state sharing, and failure handling".into(),
            category: "orchestration".into(),
        },
        AgentEntry {
            slug: "task-distributor".into(),
            name: "Task Distributor".into(),
            description: "Distributes tasks across agents, manages queues, and balances workloads".into(),
            category: "orchestration".into(),
        },
        AgentEntry {
            slug: "context-manager".into(),
            name: "Context Manager".into(),
            description: "Manages shared state and data synchronization for coordinated multi-agent access".into(),
            category: "orchestration".into(),
        },
        AgentEntry {
            slug: "knowledge-synthesizer".into(),
            name: "Knowledge Synthesizer".into(),
            description: "Extracts actionable patterns from agent interactions and enables organizational learning".into(),
            category: "orchestration".into(),
        },
        AgentEntry {
            slug: "error-coordinator".into(),
            name: "Error Coordinator".into(),
            description: "Coordinates error handling across distributed components with automated failure detection".into(),
            category: "orchestration".into(),
        },
        AgentEntry {
            slug: "workflow-orchestrator".into(),
            name: "Workflow Orchestrator".into(),
            description: "Designs and optimizes complex business process workflows with state management".into(),
            category: "orchestration".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_agents_not_empty() {
        let agents = builtin_agents();
        assert!(
            agents.len() > 100,
            "Expected 100+ agents, got {}",
            agents.len()
        );
    }

    #[test]
    fn builtin_agents_have_valid_slugs() {
        for agent in builtin_agents() {
            assert!(!agent.slug.is_empty(), "Empty slug found");
            assert!(
                !agent.name.is_empty(),
                "Empty name for slug: {}",
                agent.slug
            );
            assert!(
                !agent.description.is_empty(),
                "Empty description for slug: {}",
                agent.slug
            );
            assert!(
                !agent.category.is_empty(),
                "Empty category for slug: {}",
                agent.slug
            );
            assert!(
                agent
                    .slug
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()),
                "Invalid slug: {}",
                agent.slug
            );
        }
    }

    #[test]
    fn builtin_agents_no_duplicates() {
        let agents = builtin_agents();
        let mut slugs: Vec<&str> = agents.iter().map(|a| a.slug.as_str()).collect();
        slugs.sort();
        let original_len = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), original_len, "Duplicate slugs found");
    }
}
