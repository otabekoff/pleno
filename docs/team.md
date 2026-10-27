---
title: Team and contributors
aside: false
---

<script setup>
import { VPTeamMembers } from 'vitepress/theme'

const maintainer = [
  {
    avatar: 'https://avatars.githubusercontent.com/otabekoff?s=256',
    name: 'Otabek Sadiridinov',
    title: 'Creator and maintainer',
    links: [{ icon: 'github', link: 'https://github.com/otabekoff' }],
  },
]
</script>

# Team and contributors

## Maintainer

<VPTeamMembers size="medium" :members="maintainer" />

Pleno is written and maintained by **Otabek Sadiridinov** ([@otabekoff](https://github.com/otabekoff)). He reviews and merges changes, makes the releases and approves signing requests (see the [code signing policy](./code-signing-policy#team-roles)).

## Contributors

Everyone who has a commit in [otabekoff/pleno](https://github.com/otabekoff/pleno), from GitHub's [contributor list](https://github.com/otabekoff/pleno/graphs/contributors) when this site was built. Automated accounts such as Dependabot are left out.

<TeamContributors />

## Be listed here

Get a pull request merged into [otabekoff/pleno](https://github.com/otabekoff/pleno), and you appear in this list the next time the site is built. Code, documentation, tests and examples all count. The [contributing guide](./guide/contributing) explains how to build Pleno, run the checks and open a pull request. To start smaller, [report an issue](https://github.com/otabekoff/pleno/issues) or [share a project](./showcase) you built with Pleno.
