# Gate Project Documentation - Meta Guide

## Purpose of This File

This file serves as the entry point and instruction manual for anyone (human or LLM) working on the Gate project. It should be read first to understand both the project and how the documentation system works. This ensures consistent, high-quality contributions that maintain the project's architectural integrity.

## Documentation Philosophy

The `docs/` directory contains the authoritative specification for Gate. **Code should match documentation, not the other way around.** When making changes:

1. **Start with documentation** - Update relevant docs first to clarify your intent
2. **Implement to match** - Write code that implements the documented design
3. **Keep docs current** - Immediately update docs if implementation reveals better approaches
4. **Never let docs diverge** - Documentation debt is technical debt

## Documentation Structure

### README.md
- **Purpose**: Quick start guide for new developers
- **Content**: Setup instructions, basic commands, project status overview
- **Update frequency**: Moderate - update when setup process or basic workflows change
- **Audience**: New developers, first-time contributors

### docs/OVERVIEW.md
- **Purpose**: Highest-level project description - what Gate is, why it exists, core value proposition
- **Content**: Features, use cases, business context, user benefits, technology overview
- **Update frequency**: Rare - only for strategic repositioning, major pivots, or significant feature additions
- **Audience**: Executives, users, potential contributors, marketing material

### docs/DESIGN.md  
- **Purpose**: Technical architecture reference and component map
- **Content**: System components, data flows, security model, interfaces, file structure
- **Update frequency**: Frequent - update whenever technical decisions are made or ambiguities are discovered
- **Audience**: Developers, technical contributors, system integrators

### docs/PLAN.md
- **Purpose**: Step-by-step implementation roadmap with concrete tasks
- **Content**: Development phases, detailed tasks with code examples, testing requirements, timelines
- **Update frequency**: Continuous - check off completed tasks, update future tasks based on learnings
- **Audience**: Active developers, project managers, implementation teams

## For Code Agents

### docs/META.md (This File)
- **Purpose**: Instructions for LLMs and code agents on how to work with this project's documentation-driven development approach
- **Update frequency**: Rarely - only when documentation workflow changes
- **Audience**: LLMs, code agents, those wanting to understand the prompting approach

## Current Project Status

**Gate** is a peer-to-peer AI compute network that provides secure, private access to distributed inference resources. The project consists of:

- **Local daemon** providing OpenAI-compatible APIs
- **P2P networking** using Iroh for encrypted communication  
- **Web management interface** built with Yew
- **Public HTTPS endpoints** via relay infrastructure
- **Trust-based permissions** for secure resource sharing

**Repository Status**: **Documentation-only phase** - comprehensive design complete, but no code implementation exists yet. We are building the project incrementally following the detailed implementation plan, updating documentation as we go to ensure it stays current with reality.

## Working with This Project

### For LLMs/AI Assistants:
1. **Always read this file first** to understand project context and documentation workflow
2. **Read OVERVIEW.md** for business context and high-level architecture
3. **MUST read DESIGN.md** before writing any code - contains critical development guidelines and architecture decisions
4. **Reference PLAN.md** for implementation tasks and current progress
5. **Update documentation proactively** when making any technical decisions
6. **Maintain consistency** between all documentation files

### For Human Contributors:
1. **Start with documentation** - understand the vision before coding
2. **Propose changes via docs** - update DESIGN.md to clarify new approaches
3. **Follow the plan** - use PLAN.md as your implementation guide
4. **Document as you go** - don't accumulate documentation debt
5. **Update future tasks** - modify PLAN.md if current work reveals new requirements

### For New Contributors:
1. Read META.md (this file)
2. Read OVERVIEW.md for project understanding
3. Study DESIGN.md for technical architecture
4. Find your starting point in PLAN.md
5. Begin contributing with documentation updates

## How to Actually Start Coding

**Current State**: Empty repository with docs only - no Cargo.toml, no src/ directories, no code exists yet.

**Next Action**: Always start with the first unchecked task in PLAN.md. Currently this is:
- Phase 1, Task 1.1: Create Cargo workspace structure

**Development Environment**: 
- We use Nix for reproducible development environments
- Nix setup is included as a task in PLAN.md - will provide Rust toolchain automatically
- For now: ensure you have basic Rust toolchain installed manually

**Workflow**:
1. **Check PLAN.md**: Find first unchecked task (marked with `- [ ]`)
2. **Update docs first**: If your approach differs from documented plan, update DESIGN.md
3. **Implement**: Write code following the documented interface/structure
4. **Test**: Ensure tests pass (every task includes testing requirements)
5. **Mark complete**: Check off task in PLAN.md (`- [x]`)
6. **Update docs**: If implementation revealed better approaches, update affected docs
7. **Repeat**: Move to next unchecked task

**Key Principle**: Never let code diverge from documentation - update docs proactively to match reality.

## Documentation Maintenance Rules

### OVERVIEW.md Updates
- **Strategic changes**: New market positioning, major feature additions, business model changes
- **Refinements**: Better explanations of existing features, new use cases, improved clarity
- **Avoid**: Technical implementation details, temporary decisions, work-in-progress features

### DESIGN.md Updates  
- **Required for**: New components, changed interfaces, security decisions, protocol changes
- **Include**: Updated code examples, new data flows, architectural diagrams  
- **Keep current**: Never let implementation diverge from documented design

### PLAN.md Updates
- **Mark completed**: Check off finished tasks immediately
- **Update dependencies**: Modify future tasks when current work reveals new requirements
- **Add detail**: Expand task descriptions when more information becomes available
- **Reorder if needed**: Adjust priorities based on implementation learnings

## Quality Standards

- **Accuracy**: Documentation must reflect actual system behavior
- **Completeness**: Cover all major components and interfaces
- **Clarity**: Write for contributors who haven't seen the code
- **Consistency**: Use consistent terminology and examples across files
- **Actionability**: Provide concrete examples and implementation guidance

## Collaboration Workflow

1. **Before coding**: Update DESIGN.md with your intended approach
2. **During implementation**: Follow documented architecture and interfaces
3. **After completion**: Update PLAN.md progress and any affected future tasks
4. **When blocked**: Document the issue and potential solutions in relevant files
5. **Before commits**: Ensure all documentation accurately reflects your changes

---

**Remember**: This documentation system is designed to maintain project coherence across time and contributors. When in doubt, err on the side of over-documentation rather than under-documentation.